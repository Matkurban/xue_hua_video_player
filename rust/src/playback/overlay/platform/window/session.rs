//! Linux / Windows and macOS overlay sessions.

use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::overlay::overlay_session::{load_preroll, OverlaySession};
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;

#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
use super::backend::DesktopOverlayBackend;
#[cfg(target_os = "macos")]
use super::backend::MacosOverlayBackend;

/// Linux / Windows overlay session.
#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
#[derive(Clone)]
pub struct DesktopOverlaySession {
    last_width: Arc<AtomicI32>,
    last_height: Arc<AtomicI32>,
}

#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
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

#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
impl OverlaySession for DesktopOverlaySession {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
        !surface_overlay_ready
    }

    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface: &VideoSurface,
        _defer_log: &str,
    ) -> Result<()> {
        let gate_ready = self.gate_ready_for_load(surface.overlay_ready_for_preroll());
        load_preroll::desktop_apply_load_preroll(shell, gate_ready)
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
        stored: Arc<Mutex<Option<usize>>>,
    ) -> Result<()> {
        DesktopOverlayBackend::rebind_cached_overlay(stored.as_ref(), shell)
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

/// macOS overlay session — delegates to stateless backend helpers.
#[cfg(target_os = "macos")]
#[derive(Clone)]
pub struct MacosOverlaySession {
    last_width: Arc<AtomicI32>,
    last_height: Arc<AtomicI32>,
    overlay_sink: Option<Arc<Mutex<gst::Element>>>,
}

#[cfg(target_os = "macos")]
impl MacosOverlaySession {
    pub fn new() -> Self {
        Self {
            last_width: Arc::new(AtomicI32::new(0)),
            last_height: Arc::new(AtomicI32::new(0)),
            overlay_sink: None,
        }
    }

    pub fn set_overlay_sink(&mut self, element: gst::Element) {
        match &self.overlay_sink {
            Some(slot) => *slot.lock() = element,
            None => self.overlay_sink = Some(Arc::new(Mutex::new(element))),
        }
    }

    pub fn overlay_sink_slot(&self) -> Option<&Arc<Mutex<gst::Element>>> {
        self.overlay_sink.as_ref()
    }

    pub fn ensure_overlay_ready(&self, stored: &Mutex<Option<usize>>) -> Result<()> {
        MacosOverlayBackend::ensure_overlay_ready(stored)
    }
}

#[cfg(target_os = "macos")]
impl OverlaySession for MacosOverlaySession {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
        !surface_overlay_ready
    }

    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface: &VideoSurface,
        _defer_log: &str,
    ) -> Result<()> {
        let gate_ready = self.gate_ready_for_load(surface.overlay_ready_for_preroll());
        load_preroll::desktop_apply_load_preroll(shell, gate_ready)
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
        _shell: &PipelineShell,
        stored: Arc<Mutex<Option<usize>>>,
    ) -> Result<()> {
        let Some(slot) = self.overlay_sink.as_ref() else {
            return Ok(());
        };
        if stored.lock().is_none() {
            return Ok(());
        }
        let (width, height) = self.cached_dimensions();
        MacosOverlayBackend::schedule_rebind_on_main(stored, slot.clone(), width, height);
        Ok(())
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
        stored: Arc<Mutex<Option<usize>>>,
        _surface: VideoSurface,
        width: i32,
        height: i32,
        _play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        let Some(slot) = self.overlay_sink.as_ref() else {
            return Ok(());
        };
        MacosOverlayBackend::apply_gstreamer(&stored, &slot.lock(), width, height)
    }
}
