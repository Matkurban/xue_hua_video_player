//! Desktop overlay session — thin adapter over [`super::desktop::DesktopOverlayBackend`].

use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc,
};

use anyhow::Result;
use parking_lot::Mutex;

use crate::playback::overlay::desktop::DesktopOverlayBackend;
use crate::playback::overlay::overlay_session::{load_preroll, OverlaySession};
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;

/// Linux / Windows overlay session.
#[derive(Clone)]
pub struct DesktopOverlaySession {
    last_width: Arc<AtomicI32>,
    last_height: Arc<AtomicI32>,
}

impl DesktopOverlaySession {
    pub fn new() -> Self {
        Self {
            last_width: Arc::new(AtomicI32::new(0)),
            last_height: Arc::new(AtomicI32::new(0)),
        }
    }

    pub fn schedule_rectangle_sync(
        &self,
        stored: Arc<Mutex<Option<usize>>>,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        DesktopOverlayBackend::schedule_rectangle_sync(stored, shell, width, height);
    }
}

impl OverlaySession for DesktopOverlaySession {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
        !surface_overlay_ready
    }

    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface_overlay_ready: bool,
        _surface: &VideoSurface,
        _defer_log: &str,
    ) -> Result<()> {
        load_preroll::desktop_apply_load_preroll(shell, surface_overlay_ready)
    }

    fn is_bound(&self) -> bool {
        false
    }

    fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool {
        has_cached_handle
    }

    fn mark_shell_rebuilt(&self) {}

    fn set_cached_dimensions(&self, width: i32, height: i32) {
        if width > 0 {
            self.last_width.store(width, Ordering::SeqCst);
        }
        if height > 0 {
            self.last_height.store(height, Ordering::SeqCst);
        }
    }

    fn cached_dimensions(&self) -> (i32, i32) {
        (
            self.last_width.load(Ordering::SeqCst),
            self.last_height.load(Ordering::SeqCst),
        )
    }

    fn rebind_cached_overlay(
        &self,
        shell: &PipelineShell,
        stored: &Mutex<Option<usize>>,
    ) -> Result<()> {
        DesktopOverlayBackend::rebind_cached_overlay(stored, shell)
    }

    fn cache_notify(
        &self,
        stored: &Arc<Mutex<Option<usize>>>,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()> {
        self.set_cached_dimensions(width, height);
        if handle == 0 {
            *stored.lock() = None;
        } else {
            *stored.lock() = Some(handle as usize);
        }
        Ok(())
    }

    fn apply_gstreamer(
        &self,
        _shell: Arc<Mutex<PipelineShell>>,
        _stored: Arc<Mutex<Option<usize>>>,
        _surface: VideoSurface,
        _width: i32,
        _height: i32,
        _play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        Ok(())
    }
}
