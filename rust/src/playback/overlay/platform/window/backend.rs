//! Desktop (Win/Linux) and macOS VideoOverlay backends.

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::gst::spawn_on_gst_thread;
use crate::playback::gst::{
    clear_overlay_window_handle, expose_overlay, set_overlay_render_rectangle,
    set_overlay_window_handle,
};
use crate::playback::shell::PipelineShell;

use crate::playback::overlay::video_overlay::VideoOverlayBackend;

/// Desktop overlay operations delegated from [`crate::playback::surface::VideoSurface`].
#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
pub struct DesktopOverlayBackend;

#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
impl VideoOverlayBackend for DesktopOverlayBackend {
    fn stored_handle(&self) -> &Mutex<Option<usize>> {
        unreachable!(
            "DesktopOverlayBackend is a stateless delegate; use VideoSurface stored handle"
        )
    }
}

#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
pub fn apply_overlay_handle(
    video_sink: &gst::Element,
    handle: usize,
    stored: &Mutex<Option<usize>>,
) -> Result<()> {
    if handle == 0 {
        stored.lock().take();
    } else {
        *stored.lock() = Some(handle);
    }

    if handle == 0 {
        clear_overlay_window_handle(video_sink)?;
    } else {
        set_overlay_window_handle(video_sink, handle)?;
    }
    Ok(())
}

#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
impl DesktopOverlayBackend {
    pub fn rebind_cached_overlay(
        stored: &Mutex<Option<usize>>,
        shell: &PipelineShell,
    ) -> Result<()> {
        if let Some(handle) = *stored.lock() {
            apply_overlay_handle(shell.video_sink(), handle, stored)?;
        }
        Ok(())
    }

    pub fn schedule_rectangle_sync(
        stored: Arc<Mutex<Option<usize>>>,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        spawn_on_gst_thread(move || {
            let guard = shell.lock();
            if width > 0 && height > 0 {
                guard.apply_overlay_render_rectangle(width, height);
            } else if stored.lock().is_some() {
                guard.expose_video_overlay();
            }
        });
    }
}

/// macOS overlay operations delegated from [`crate::playback::surface::VideoSurface`].
#[cfg(target_os = "macos")]
pub struct MacosOverlayBackend;

#[cfg(target_os = "macos")]
impl VideoOverlayBackend for MacosOverlayBackend {
    fn stored_handle(&self) -> &Mutex<Option<usize>> {
        unreachable!("MacosOverlayBackend is a stateless delegate; use VideoSurface stored handle")
    }
}

#[cfg(target_os = "macos")]
impl MacosOverlayBackend {
    pub fn apply_gstreamer(
        stored: &Mutex<Option<usize>>,
        sink: &gst::Element,
        width: i32,
        height: i32,
    ) -> Result<()> {
        match *stored.lock() {
            None => clear_overlay_window_handle(sink),
            Some(handle) => {
                set_overlay_window_handle(sink, handle)?;
                if width > 0 && height > 0 {
                    set_overlay_render_rectangle(sink, width, height);
                }
                Ok(())
            }
        }
    }

    pub fn ensure_overlay_ready(stored: &Mutex<Option<usize>>) -> Result<()> {
        if stored.lock().is_none() {
            log::warn!(
                "macOS overlay handle not cached yet; playback may fail until platform view binds"
            );
        }
        Ok(())
    }
}
