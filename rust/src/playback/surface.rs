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
