//! Linux / Windows and macOS overlay sessions.

use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    Arc,
};

use anyhow::Result;
#[cfg(target_os = "macos")]
use gstreamer as gst;
use parking_lot::Mutex;

#[cfg(target_os = "macos")]
use crate::gst::spawn_on_gst_thread;
use crate::playback::overlay::overlay_session::{load_preroll, OverlaySession};
#[cfg(target_os = "macos")]
use crate::playback::play_resume::maybe_resume_after_overlay_bind;
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
    overlay_bound: Arc<AtomicBool>,
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
            overlay_bound: Arc::new(AtomicBool::new(false)),
            last_width: Arc::new(AtomicI32::new(0)),
            last_height: Arc::new(AtomicI32::new(0)),
        }
    }

    pub fn set_bound(&self, bound: bool) {
        self.overlay_bound.store(bound, Ordering::SeqCst);
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

    /// Applies the window handle and marks the overlay bound when `handle != 0`.
    pub fn apply_window_handle(
        &self,
        shell: &PipelineShell,
        stored: &Mutex<Option<usize>>,
        window_handle: i64,
    ) -> Result<()> {
        super::backend::apply_overlay_handle(
            shell.video_sink(),
            window_handle as usize,
            stored,
        )?;
        self.set_bound(window_handle != 0);
        Ok(())
    }
}

#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
impl OverlaySession for DesktopOverlaySession {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
        // Match Android: only preroll after VideoOverlay is bound (GStreamer
        // tutorials require set_window_handle before / as PAUSED is reached).
        surface_overlay_ready
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
        self.overlay_bound.load(Ordering::SeqCst)
    }

    fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool {
        has_cached_handle && self.is_bound()
    }

    fn mark_shell_rebuilt(&self) {
        self.set_bound(false);
    }

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
        DesktopOverlayBackend::rebind_cached_overlay(stored.as_ref(), shell)?;
        self.set_bound(stored.lock().is_some());
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
            self.set_bound(false);
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
    overlay_bound: Arc<AtomicBool>,
    last_width: Arc<AtomicI32>,
    last_height: Arc<AtomicI32>,
    overlay_sink: Option<Arc<Mutex<gst::Element>>>,
}

#[cfg(target_os = "macos")]
impl MacosOverlaySession {
    pub fn new() -> Self {
        Self {
            overlay_bound: Arc::new(AtomicBool::new(false)),
            last_width: Arc::new(AtomicI32::new(0)),
            last_height: Arc::new(AtomicI32::new(0)),
            overlay_sink: None,
        }
    }

    pub fn set_bound(&self, bound: bool) {
        self.overlay_bound.store(bound, Ordering::SeqCst);
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

    /// Binds on the main thread, then resumes play on `xhvp-gst` when desired.
    pub fn apply_and_maybe_resume(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
        surface: VideoSurface,
        width: i32,
        height: i32,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        let Some(slot) = self.overlay_sink.as_ref() else {
            log::warn!("macOS overlay apply: no overlay_sink slot yet");
            return Ok(());
        };
        MacosOverlayBackend::apply_gstreamer(&stored, &slot.lock(), width, height)?;
        let bound = stored.lock().is_some();
        self.set_bound(bound);
        if !bound {
            return Ok(());
        }
        let intent = play_intent.clone_for_async();
        spawn_on_gst_thread(move || {
            if let Err(e) =
                maybe_resume_after_overlay_bind(shell, &intent.replay, &intent.swap, &surface)
            {
                log::warn!("macOS overlay bind resume: {e:#}");
            }
        });
        Ok(())
    }
}

#[cfg(target_os = "macos")]
impl OverlaySession for MacosOverlaySession {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
        // Match Android: only preroll after VideoOverlay is bound.
        surface_overlay_ready
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
        self.overlay_bound.load(Ordering::SeqCst)
    }

    fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool {
        has_cached_handle && self.is_bound()
    }

    fn mark_shell_rebuilt(&self) {
        self.set_bound(false);
    }

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
            self.set_bound(false);
            return Ok(());
        }
        let (width, height) = self.cached_dimensions();
        MacosOverlayBackend::rebind_on_main_sync(stored.clone(), slot.clone(), width, height);
        self.set_bound(stored.lock().is_some());
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
            self.set_bound(false);
        } else {
            *stored.lock() = Some(handle as usize);
            // Cache alone is not a GStreamer bind — clear until apply_gstreamer.
            self.set_bound(false);
        }
        Ok(())
    }

    fn apply_gstreamer(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
        surface: VideoSurface,
        width: i32,
        height: i32,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        self.apply_and_maybe_resume(shell, stored, surface, width, height, play_intent)
    }
}

#[cfg(all(test, not(target_os = "android"), not(target_os = "ios")))]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_bound_requires_apply_not_cache_alone() {
        let session = MacosOverlaySession::new();
        assert!(!session.is_bound());
        assert!(!session.overlay_ready_for_preroll(true));
        session.set_bound(true);
        assert!(session.is_bound());
        assert!(session.overlay_ready_for_preroll(true));
        session.mark_shell_rebuilt();
        assert!(!session.is_bound());
    }

    #[cfg(all(
        not(target_os = "macos"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    #[test]
    fn desktop_bound_tracks_apply() {
        let session = DesktopOverlaySession::new();
        assert!(!session.is_bound());
        assert!(!session.overlay_ready_for_preroll(true));
        session.set_bound(true);
        assert!(session.overlay_ready_for_preroll(true));
        session.mark_shell_rebuilt();
        assert!(!session.is_bound());
    }
}
