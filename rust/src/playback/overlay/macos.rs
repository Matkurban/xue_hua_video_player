//! macOS VideoOverlay backend (`osxvideosink`).

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

use crate::video::{
    clear_overlay_window_handle, set_overlay_render_rectangle, set_overlay_window_handle,
};

use super::video_overlay::VideoOverlayBackend;

/// macOS overlay operations delegated from [`super::super::surface::VideoSurface`].
pub struct MacosOverlayBackend;

impl VideoOverlayBackend for MacosOverlayBackend {
    fn stored_handle(&self) -> &Mutex<Option<usize>> {
        unreachable!("MacosOverlayBackend is a stateless delegate; use VideoSurface stored handle")
    }
}

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
