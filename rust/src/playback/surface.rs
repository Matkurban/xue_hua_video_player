use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::gst_runtime::spawn_on_gst_thread;
use crate::playback::shell::{PipelineShell, SourceKind};
use crate::playback::state::set_state_sync;
use crate::video::{
    clear_overlay_window_handle, expose_overlay, set_overlay_render_rectangle,
    set_overlay_window_handle,
};

/// Cached native overlay handle and platform-specific bind helpers.
pub struct VideoSurface {
    stored: std::sync::Arc<Mutex<Option<usize>>>,
    #[cfg(target_os = "macos")]
    overlay_sink: Option<std::sync::Arc<Mutex<gst::Element>>>,
}

impl VideoSurface {
    pub fn new(stored: std::sync::Arc<Mutex<Option<usize>>>) -> Self {
        Self {
            stored,
            #[cfg(target_os = "macos")]
            overlay_sink: None,
        }
    }

    #[cfg(target_os = "macos")]
    pub fn with_macos_overlay_sink(
        stored: std::sync::Arc<Mutex<Option<usize>>>,
        overlay_sink: std::sync::Arc<Mutex<gst::Element>>,
    ) -> Self {
        Self {
            stored,
            overlay_sink: Some(overlay_sink),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn set_macos_overlay_sink(&mut self, element: gst::Element) {
        match &self.overlay_sink {
            Some(slot) => *slot.lock() = element,
            None => {
                self.overlay_sink = Some(std::sync::Arc::new(Mutex::new(element)));
            }
        }
    }

    pub fn stored_handle(&self) -> std::sync::Arc<Mutex<Option<usize>>> {
        self.stored.clone()
    }

    pub fn has_cached_handle(&self) -> bool {
        self.stored.lock().is_some()
    }

    pub fn cache_handle(&self, handle: usize) {
        if handle == 0 {
            self.stored.lock().take();
        } else {
            *self.stored.lock() = Some(handle);
        }
    }

    #[cfg(target_os = "macos")]
    pub fn cache_macos_handle(&self, view_ptr: i64) {
        if view_ptr == 0 {
            self.stored.lock().take();
        } else {
            *self.stored.lock() = Some(view_ptr as usize);
        }
    }

    #[cfg(target_os = "macos")]
    pub fn macos_overlay_sink(&self) -> Option<&std::sync::Arc<Mutex<gst::Element>>> {
        self.overlay_sink.as_ref()
    }

    #[cfg(target_os = "macos")]
    pub fn apply_macos_overlay_gstreamer(&self, width: i32, height: i32) -> Result<()> {
        let Some(slot) = self.overlay_sink.as_ref() else {
            return Ok(());
        };
        let sink = slot.lock().clone();
        match *self.stored.lock() {
            None => clear_overlay_window_handle(&sink),
            Some(handle) => {
                set_overlay_window_handle(&sink, handle)?;
                if width > 0 && height > 0 {
                    set_overlay_render_rectangle(&sink, width, height);
                }
                Ok(())
            }
        }
    }

    #[cfg(target_os = "macos")]
    pub fn ensure_overlay_ready(&self) -> Result<()> {
        if self.stored.lock().is_none() {
            log::warn!(
                "macOS overlay handle not cached yet; playback may open a standalone window"
            );
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn ensure_overlay_ready(&self) -> Result<()> {
        Ok(())
    }

    pub fn rebind_cached_overlay(&self, shell: &PipelineShell) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let _ = (self, shell);
            return Ok(());
        }
        #[cfg(not(target_os = "macos"))]
        {
            if let Some(handle) = *self.stored.lock() {
                apply_overlay_handle(&shell.video_sink, handle, &self.stored)?;
            }
            Ok(())
        }
    }

    #[cfg(target_os = "android")]
    pub fn notify_android_surface(
        &self,
        shell: std::sync::Arc<Mutex<PipelineShell>>,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()> {
        if handle == 0 {
            cache_android_native_window(&self.stored, 0)?;
            spawn_on_gst_thread(move || {
                let guard = shell.lock();
                if let Err(e) = clear_overlay_window_handle(&guard.video_sink) {
                    log::warn!("android overlay clear: {e:#}");
                }
            });
            return Ok(());
        }
        cache_android_native_window(&self.stored, handle as usize)?;
        schedule_android_overlay_apply(shell, self.stored.clone(), width, height);
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    pub fn set_window_handle_on_gst(
        &self,
        shell: &mut PipelineShell,
        window_handle: i64,
    ) -> Result<()> {
        let handle = window_handle as usize;
        apply_overlay_handle(&shell.video_sink, handle, &self.stored)
    }

    /// Applies render rectangle + expose on the Gst thread (iOS / desktop).
    #[cfg(not(any(target_os = "android", target_os = "macos")))]
    pub fn schedule_overlay_rectangle_sync(
        &self,
        shell: std::sync::Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        let stored = self.stored.clone();
        spawn_on_gst_thread(move || {
            let guard = shell.lock();
            if width > 0 && height > 0 {
                set_overlay_render_rectangle(&guard.video_sink, width, height);
            } else if stored.lock().is_some() {
                expose_overlay(&guard.video_sink);
            }
        });
    }

    /// Shares overlay state for [`super::switch::SwitchContext`].
    pub fn clone_for_switch(&self) -> Self {
        Self {
            stored: self.stored.clone(),
            #[cfg(target_os = "macos")]
            overlay_sink: self.overlay_sink.clone(),
        }
    }
}

pub fn apply_overlay_handle(
    video_sink: &gst::Element,
    handle: usize,
    stored: &Mutex<Option<usize>>,
) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        if handle == 0 {
            cache_android_native_window(stored, 0)?;
            clear_overlay_window_handle(video_sink)?;
            return Ok(());
        }
        cache_android_native_window(stored, handle)?;
    }

    #[cfg(not(target_os = "android"))]
    {
        if handle == 0 {
            stored.lock().take();
        } else {
            *stored.lock() = Some(handle);
        }
    }

    if handle == 0 {
        clear_overlay_window_handle(video_sink)?;
    } else {
        set_overlay_window_handle(video_sink, handle)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn assign_overlay_sink(slot: &std::sync::Arc<Mutex<gst::Element>>, element: &gst::Element) {
    *slot.lock() = element.clone();
}

#[cfg(target_os = "android")]
pub fn cache_android_native_window(stored: &Mutex<Option<usize>>, handle: usize) -> Result<()> {
    if handle == 0 {
        if let Some(old) = stored.lock().take() {
            crate::platform_view_android::release_native_window(old);
        }
        return Ok(());
    }
    let mut guard = stored.lock();
    if let Some(old) = *guard {
        if old != handle {
            crate::platform_view_android::release_native_window(old);
        }
    }
    *guard = Some(handle);
    Ok(())
}

#[cfg(target_os = "android")]
pub fn apply_android_overlay_on_gst(
    shell: &mut PipelineShell,
    handle: usize,
    width: i32,
    height: i32,
) -> Result<()> {
    set_overlay_window_handle(&shell.video_sink, handle)?;
    if width > 0 && height > 0 {
        set_overlay_render_rectangle(&shell.video_sink, width, height);
    }
    expose_overlay(&shell.video_sink);
    maybe_preroll_after_overlay_bind(shell)
}

pub fn maybe_preroll_after_overlay_bind(shell: &PipelineShell) -> Result<()> {
    let (_, current, pending) = shell.pipeline.state(gst::ClockTime::ZERO);
    if pending != gst::State::VoidPending {
        return Ok(());
    }
    if current != gst::State::Ready {
        return Ok(());
    }
    match shell.kind {
        SourceKind::Uri => {
            let uri: String = shell.pipeline.property("uri");
            if uri.is_empty() {
                return Ok(());
            }
        }
        SourceKind::Asset => {}
    }
    #[cfg(target_os = "android")]
    crate::diag::logcat_info("gst: overlay bound — starting Paused preroll");
    set_state_sync(&shell.pipeline, gst::State::Paused)
}

#[cfg(target_os = "android")]
pub fn schedule_android_overlay_apply(
    bundle: std::sync::Arc<Mutex<PipelineShell>>,
    stored: std::sync::Arc<Mutex<Option<usize>>>,
    width: i32,
    height: i32,
) {
    spawn_on_gst_thread(move || {
        let mut guard = bundle.lock();
        let Some(handle) = *stored.lock() else {
            return;
        };
        if let Err(e) = apply_android_overlay_on_gst(&mut guard, handle, width, height) {
            log::warn!("android overlay apply: {e:#}");
        }
    });
}
