//! Android VideoOverlay helpers (`glimagesink` / native window).

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::shell::PipelineShell;
use crate::video::expose_overlay;

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

/// Rebinds the cached native window on the Gst thread (Android).
pub fn refresh_mobile_overlay_on_gst(
    shell: &PipelineShell,
    handle: usize,
    width: i32,
    height: i32,
    reason: &str,
) -> Result<()> {
    shell.apply_overlay_window_handle(handle)?;
    shell.apply_overlay_render_rectangle(width, height);
    shell.expose_video_overlay();
    crate::diag::logcat_info(&format!("gst: overlay refresh {reason} ({width}x{height})"));
    Ok(())
}

/// Paused preroll + overlay refresh — shared by load and bind paths.
pub fn android_pause_preroll_with_refresh(
    shell: &PipelineShell,
    surface: &crate::playback::surface::VideoSurface,
    log_prefix: Option<&str>,
) -> Result<()> {
    if let Some(msg) = log_prefix {
        crate::diag::logcat_info(msg);
    }
    shell.set_state_sync(gst::State::Paused)?;
    if let Some(handle) = *surface.stored_handle().lock() {
        let (width, height) = surface.cached_dimensions();
        refresh_mobile_overlay_on_gst(shell, handle, width, height, "after Paused preroll")?;
    }
    Ok(())
}
