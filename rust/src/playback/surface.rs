use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::gst_runtime::spawn_on_gst_thread;
use crate::playback::shell::{PipelineShell, SourceKind};
use crate::playback::switch::SwitchContext;
#[cfg(target_os = "android")]
use crate::playback::state::{resume_or_replay_from_eos, set_state_sync};
#[cfg(not(target_os = "android"))]
use crate::playback::state::set_state_sync;
use crate::video::{
    clear_overlay_window_handle, expose_overlay, set_overlay_render_rectangle,
    set_overlay_window_handle,
};

/// Play intent forwarded to the Gst thread when an Android overlay binds.
#[cfg(target_os = "android")]
pub struct AndroidOverlayPlayIntent {
    pub desired_playing: Arc<AtomicBool>,
    pub at_eos: Arc<AtomicBool>,
    pub switch_ctx: SwitchContext,
}

/// Cached native overlay handle and platform-specific bind helpers.
pub struct VideoSurface {
    stored: std::sync::Arc<Mutex<Option<usize>>>,
    #[cfg(target_os = "android")]
    overlay_bound: Arc<AtomicBool>,
    #[cfg(target_os = "android")]
    last_width: Arc<AtomicI32>,
    #[cfg(target_os = "android")]
    last_height: Arc<AtomicI32>,
    #[cfg(target_os = "macos")]
    overlay_sink: Option<std::sync::Arc<Mutex<gst::Element>>>,
}

impl VideoSurface {
    pub fn new(stored: std::sync::Arc<Mutex<Option<usize>>>) -> Self {
        Self {
            stored,
            #[cfg(target_os = "android")]
            overlay_bound: Arc::new(AtomicBool::new(false)),
            #[cfg(target_os = "android")]
            last_width: Arc::new(AtomicI32::new(0)),
            #[cfg(target_os = "android")]
            last_height: Arc::new(AtomicI32::new(0)),
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
            #[cfg(target_os = "android")]
            overlay_bound: Arc::new(AtomicBool::new(false)),
            #[cfg(target_os = "android")]
            last_width: Arc::new(AtomicI32::new(0)),
            #[cfg(target_os = "android")]
            last_height: Arc::new(AtomicI32::new(0)),
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

    /// True once `glimagesink` received `set_window_handle` on the Gst thread (Android).
    #[cfg(target_os = "android")]
    pub fn is_overlay_bound_on_gst(&self) -> bool {
        self.overlay_bound.load(Ordering::SeqCst)
    }

    #[cfg(not(target_os = "android"))]
    pub fn is_overlay_bound_on_gst(&self) -> bool {
        self.has_cached_handle()
    }

    /// True when a cached native handle exists and overlay was rebound on the Gst thread.
    #[cfg(target_os = "android")]
    pub fn overlay_ready_for_preroll(&self) -> bool {
        self.has_cached_handle() && self.is_overlay_bound_on_gst()
    }

    #[cfg(not(target_os = "android"))]
    pub fn overlay_ready_for_preroll(&self) -> bool {
        self.has_cached_handle()
    }

    /// Clears overlay-bound state when the pipeline shell's video sink is replaced.
    #[cfg(target_os = "android")]
    pub fn mark_shell_rebuilt(&self) {
        self.overlay_bound.store(false, Ordering::SeqCst);
    }

    #[cfg(not(target_os = "android"))]
    pub fn mark_shell_rebuilt(&self) {}

    #[cfg(target_os = "android")]
    pub fn set_cached_dimensions(&self, width: i32, height: i32) {
        if width > 0 {
            self.last_width.store(width, Ordering::SeqCst);
        }
        if height > 0 {
            self.last_height.store(height, Ordering::SeqCst);
        }
    }

    #[cfg(target_os = "android")]
    pub fn cached_dimensions(&self) -> (i32, i32) {
        (
            self.last_width.load(Ordering::SeqCst),
            self.last_height.load(Ordering::SeqCst),
        )
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
        #[cfg(target_os = "android")]
        {
            if let Some(handle) = *self.stored.lock() {
                let (width, height) = self.cached_dimensions();
                refresh_android_overlay_on_gst(shell, handle, width, height, "rebind")?;
                self.overlay_bound.store(true, Ordering::SeqCst);
                crate::diag::logcat_info("gst: overlay rebound on new video_sink");
            }
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "android")))]
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
        play_intent: AndroidOverlayPlayIntent,
    ) -> Result<()> {
        if handle == 0 {
            self.overlay_bound.store(false, Ordering::SeqCst);
            cache_android_native_window(&self.stored, 0)?;
            let overlay_bound = self.overlay_bound.clone();
            spawn_on_gst_thread(move || {
                let guard = shell.lock();
                if let Err(e) = clear_overlay_window_handle(&guard.video_sink) {
                    log::warn!("android overlay clear: {e:#}");
                }
                overlay_bound.store(false, Ordering::SeqCst);
            });
            return Ok(());
        }
        self.set_cached_dimensions(width, height);
        self.overlay_bound.store(false, Ordering::SeqCst);
        cache_android_native_window(&self.stored, handle as usize)?;
        schedule_android_overlay_apply(
            shell,
            self.stored.clone(),
            self.overlay_bound.clone(),
            width,
            height,
            play_intent,
        );
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

    /// Re-applies VideoOverlay render rectangle + expose after surface resize (Android).
    #[cfg(target_os = "android")]
    pub fn schedule_android_overlay_rectangle_sync(
        &self,
        shell: std::sync::Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        self.set_cached_dimensions(width, height);
        let stored = self.stored.clone();
        spawn_on_gst_thread(move || {
            let guard = shell.lock();
            let Some(handle) = *stored.lock() else {
                return;
            };
            if let Err(e) = refresh_android_overlay_on_gst(
                &guard,
                handle,
                width,
                height,
                "surface resize",
            ) {
                crate::diag::logcat_error(&format!("android overlay resize: {e:#}"));
            }
        });
    }

    /// Shares overlay state for [`super::switch::SwitchContext`].
    pub fn clone_for_switch(&self) -> Self {
        Self {
            stored: self.stored.clone(),
            #[cfg(target_os = "android")]
            overlay_bound: self.overlay_bound.clone(),
            #[cfg(target_os = "android")]
            last_width: self.last_width.clone(),
            #[cfg(target_os = "android")]
            last_height: self.last_height.clone(),
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

/// Rebinds the cached native window, sets render rectangle, and double-exposes (Android Gst thread).
#[cfg(target_os = "android")]
pub fn refresh_android_overlay_on_gst(
    shell: &PipelineShell,
    handle: usize,
    width: i32,
    height: i32,
    reason: &str,
) -> Result<()> {
    set_overlay_window_handle(&shell.video_sink, handle)?;
    if width > 0 && height > 0 {
        set_overlay_render_rectangle(&shell.video_sink, width, height);
    }
    expose_overlay(&shell.video_sink);
    crate::diag::logcat_info(&format!(
        "gst: overlay refresh {reason} ({width}x{height})"
    ));
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn refresh_android_overlay_on_gst(
    _shell: &PipelineShell,
    _handle: usize,
    _width: i32,
    _height: i32,
    _reason: &str,
) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "android")]
pub fn apply_android_overlay_on_gst(
    shell: &mut PipelineShell,
    handle: usize,
    width: i32,
    height: i32,
    play_intent: &AndroidOverlayPlayIntent,
    overlay_bound: &AtomicBool,
) -> Result<()> {
    refresh_android_overlay_on_gst(shell, handle, width, height, "surface bind")?;
    overlay_bound.store(true, Ordering::SeqCst);
    let (_, current, pending) = shell.pipeline.state(gst::ClockTime::ZERO);
    crate::diag::logcat_info(&format!(
        "gst: overlay applied on Gst thread — pipeline {current:?} pending {pending:?}"
    ));
    maybe_preroll_and_resume_play(shell, play_intent, handle, width, height)
}

fn shell_has_pending_media(shell: &PipelineShell) -> bool {
    match shell.kind {
        SourceKind::Uri => {
            let uri: String = shell.pipeline.property("uri");
            !uri.is_empty()
        }
        SourceKind::Asset => true,
    }
}

#[cfg(target_os = "android")]
fn maybe_preroll_and_resume_play(
    shell: &mut PipelineShell,
    play_intent: &AndroidOverlayPlayIntent,
    handle: usize,
    width: i32,
    height: i32,
) -> Result<()> {
    let (_, current, pending) = shell.pipeline.state(gst::ClockTime::ZERO);
    let desired = play_intent.desired_playing.load(Ordering::SeqCst);

    if pending != gst::State::VoidPending {
        crate::diag::logcat_info(&format!(
            "gst: overlay bind — pipeline pending {pending:?}, current {current:?}"
        ));
        if desired && current == gst::State::Paused {
            crate::diag::logcat_info("gst: overlay bound — resuming play while pending");
            resume_or_replay_from_eos(shell, &play_intent.at_eos, Some(&play_intent.switch_ctx))?;
            return refresh_android_overlay_on_gst(shell, handle, width, height, "after Playing");
        }
        return Ok(());
    }

    if current == gst::State::Ready && shell_has_pending_media(shell) {
        crate::diag::logcat_info("gst: overlay bound — starting Paused preroll");
        set_state_sync(&shell.pipeline, gst::State::Paused)?;
        refresh_android_overlay_on_gst(shell, handle, width, height, "after Paused preroll")?;
    } else if current != gst::State::Ready && current != gst::State::Paused {
        return Ok(());
    }

    if desired {
        crate::diag::logcat_info("gst: overlay bound — resuming play (desired_playing=true)");
        resume_or_replay_from_eos(shell, &play_intent.at_eos, Some(&play_intent.switch_ctx))?;
        refresh_android_overlay_on_gst(shell, handle, width, height, "after Playing")?;
    }
    Ok(())
}

pub fn maybe_preroll_after_overlay_bind(shell: &PipelineShell) -> Result<()> {
    let (_, current, pending) = shell.pipeline.state(gst::ClockTime::ZERO);
    if pending != gst::State::VoidPending {
        return Ok(());
    }
    if current != gst::State::Ready {
        return Ok(());
    }
    if !shell_has_pending_media(shell) {
        return Ok(());
    }
    #[cfg(target_os = "android")]
    crate::diag::logcat_info("gst: overlay bound — starting Paused preroll");
    set_state_sync(&shell.pipeline, gst::State::Paused)
}

#[cfg(target_os = "android")]
pub fn schedule_android_overlay_apply(
    bundle: std::sync::Arc<Mutex<PipelineShell>>,
    stored: std::sync::Arc<Mutex<Option<usize>>>,
    overlay_bound: Arc<AtomicBool>,
    width: i32,
    height: i32,
    play_intent: AndroidOverlayPlayIntent,
) {
    spawn_on_gst_thread(move || {
        let mut guard = bundle.lock();
        let Some(handle) = *stored.lock() else {
            overlay_bound.store(false, Ordering::SeqCst);
            return;
        };
        if let Err(e) = apply_android_overlay_on_gst(
            &mut guard,
            handle,
            width,
            height,
            &play_intent,
            &overlay_bound,
        ) {
            overlay_bound.store(false, Ordering::SeqCst);
            crate::diag::logcat_error(&format!("android overlay apply: {e:#}"));
        }
    });
}
