//! Linux / Windows VideoOverlay backend (`xvimagesink` / `d3dvideosink`).

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::gst_runtime::spawn_on_gst_thread;
use crate::playback::shell::PipelineShell;
use crate::video::{
    clear_overlay_window_handle, expose_overlay, set_overlay_render_rectangle,
    set_overlay_window_handle,
};

use super::video_overlay::VideoOverlayBackend;

/// Desktop overlay operations delegated from [`super::super::surface::VideoSurface`].
pub struct DesktopOverlayBackend;

impl VideoOverlayBackend for DesktopOverlayBackend {
    fn stored_handle(&self) -> &Mutex<Option<usize>> {
        unreachable!(
            "DesktopOverlayBackend is a stateless delegate; use VideoSurface stored handle"
        )
    }
}

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

impl DesktopOverlayBackend {
    pub fn rebind_cached_overlay(
        stored: &Mutex<Option<usize>>,
        shell: &PipelineShell,
    ) -> Result<()> {
        if let Some(handle) = *stored.lock() {
            apply_overlay_handle(&shell.video_sink, handle, stored)?;
        }
        Ok(())
    }

    pub fn schedule_rectangle_sync(
        stored: Arc<Mutex<Option<usize>>>,
        shell: Arc<parking_lot::Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        spawn_on_gst_thread(move || {
            let guard = shell.lock();
            if width > 0 && height > 0 {
                set_overlay_render_rectangle(&guard.video_sink, width, height);
            } else if stored.lock().is_some() {
                expose_overlay(&guard.video_sink);
            }
        });
    }
}
