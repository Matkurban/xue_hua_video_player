//! Android VideoOverlay helpers (`glimagesink` / native window).

use anyhow::Result;
use parking_lot::Mutex;

use crate::playback::shell::PipelineShell;
use crate::video::{
    expose_overlay, set_overlay_render_rectangle, set_overlay_window_handle,
};

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
    set_overlay_window_handle(&shell.video_sink, handle)?;
    if width > 0 && height > 0 {
        set_overlay_render_rectangle(&shell.video_sink, width, height);
    }
    expose_overlay(&shell.video_sink);
    crate::diag::logcat_info(&format!("gst: overlay refresh {reason} ({width}x{height})"));
    Ok(())
}
