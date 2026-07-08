//! Unified overlay session interface — load preroll + surface notify/apply.

use std::sync::Arc;

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::overlay::preroll::{decide_preroll_action, PrerollAction};
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;

/// Platform overlay session — load preroll, notify cache, and Gst apply/attach.
pub trait OverlaySession: Send + Sync {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool;

    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface: &VideoSurface,
        defer_log: &str,
    ) -> Result<()>;

    fn is_bound(&self) -> bool;

    fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool;

    fn mark_shell_rebuilt(&self);

    fn set_cached_dimensions(&self, width: i32, height: i32);

    fn cached_dimensions(&self) -> (i32, i32);

    fn rebind_cached_overlay(
        &self,
        shell: &PipelineShell,
        stored: Arc<Mutex<Option<usize>>>,
    ) -> Result<()>;

    /// Cache native handle and dimensions (no Gst attach on iOS/macOS).
    fn cache_notify(
        &self,
        stored: &Arc<Mutex<Option<usize>>>,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()>;

    /// Layout-time Gst overlay apply/attach.
    fn apply_gstreamer(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
        surface: VideoSurface,
        width: i32,
        height: i32,
        play_intent: OverlayPlayIntent,
    ) -> Result<()>;

    /// Android JNI entry: cache + schedule bind in one call.
    fn notify_surface_with_shell(
        &self,
        stored: Arc<Mutex<Option<usize>>>,
        handle: i64,
        width: i32,
        height: i32,
        shell: Arc<Mutex<PipelineShell>>,
        surface: VideoSurface,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        let _ = (stored, handle, width, height, shell, surface, play_intent);
        Ok(())
    }
}

/// Shared load-preroll helpers used by platform session adapters.
pub(crate) mod load_preroll {
    use super::*;

    #[cfg(target_os = "android")]
    pub fn android_apply_load_preroll(
        shell: &PipelineShell,
        gate_ready: bool,
        surface: &VideoSurface,
        defer_log: &str,
    ) -> Result<()> {
        use crate::playback::overlay::platform::android::android_pause_preroll_with_refresh;

        let snapshot = shell.snapshot();
        match decide_preroll_action(snapshot, false, gate_ready) {
            PrerollAction::PausePreroll => {
                android_pause_preroll_with_refresh(shell, surface, None)?;
            }
            PrerollAction::Defer => {
                crate::diag::logcat_info(defer_log);
            }
            PrerollAction::Noop | PrerollAction::ResumePlaying => {}
        }
        Ok(())
    }

    pub fn ios_apply_load_preroll(gate_ready: bool, defer_log: &str) -> Result<()> {
        if gate_ready {
            log::debug!("gst: ios layer attach deferred to IosOverlaySession after load");
        } else {
            log::info!("{defer_log}");
        }
        Ok(())
    }

    pub fn desktop_apply_load_preroll(shell: &PipelineShell, gate_ready: bool) -> Result<()> {
        let snapshot = shell.snapshot();
        if decide_preroll_action(snapshot, false, gate_ready) == PrerollAction::PausePreroll {
            shell.set_state_sync(gst::State::Paused)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod fake {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    /// Test double for overlay session policy methods.
    pub struct FakeOverlaySession {
        pub bound: AtomicBool,
        pub gate_ready: bool,
        pub preroll_ready: bool,
    }

    impl FakeOverlaySession {
        pub fn new(gate_ready: bool, preroll_ready: bool) -> Self {
            Self {
                bound: AtomicBool::new(false),
                gate_ready,
                preroll_ready,
            }
        }
    }

    impl OverlaySession for FakeOverlaySession {
        fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
            if self.gate_ready {
                surface_overlay_ready
            } else {
                false
            }
        }

        fn apply_load_preroll(
            &self,
            _shell: &PipelineShell,
            _surface: &VideoSurface,
            _defer_log: &str,
        ) -> Result<()> {
            Ok(())
        }

        fn is_bound(&self) -> bool {
            self.bound.load(Ordering::SeqCst)
        }

        fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool {
            has_cached_handle && self.preroll_ready && self.is_bound()
        }

        fn mark_shell_rebuilt(&self) {
            self.bound.store(false, Ordering::SeqCst);
        }

        fn set_cached_dimensions(&self, _width: i32, _height: i32) {}

        fn cached_dimensions(&self) -> (i32, i32) {
            (0, 0)
        }

        fn rebind_cached_overlay(
            &self,
            _shell: &PipelineShell,
            _stored: Arc<Mutex<Option<usize>>>,
        ) -> Result<()> {
            Ok(())
        }

        fn cache_notify(
            &self,
            _stored: &Arc<Mutex<Option<usize>>>,
            _handle: i64,
            _width: i32,
            _height: i32,
        ) -> Result<()> {
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
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::fake::FakeOverlaySession;
    use super::*;

    #[test]
    fn fake_gate_passes_surface_ready_when_configured() {
        let session = FakeOverlaySession::new(true, true);
        assert!(session.gate_ready_for_load(true));
        assert!(!session.gate_ready_for_load(false));
    }

    #[test]
    fn fake_gate_always_false_when_configured() {
        let session = FakeOverlaySession::new(false, true);
        assert!(!session.gate_ready_for_load(true));
        assert!(!session.gate_ready_for_load(false));
    }

    #[test]
    fn fake_preroll_requires_bind_when_configured() {
        let session = FakeOverlaySession::new(true, true);
        assert!(!session.overlay_ready_for_preroll(true));
        session.bound.store(true, Ordering::SeqCst);
        assert!(session.overlay_ready_for_preroll(true));
    }

    #[cfg(all(
        not(target_os = "android"),
        not(target_os = "ios"),
        not(target_os = "macos")
    ))]
    #[test]
    fn desktop_gate_inverts_ready() {
        use crate::playback::overlay::DesktopOverlaySession;
        let session = DesktopOverlaySession::new();
        assert!(session.gate_ready_for_load(false));
        assert!(!session.gate_ready_for_load(true));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_gate_inverts_ready() {
        use crate::playback::overlay::MacosOverlaySession;
        let session = MacosOverlaySession::new();
        assert!(session.gate_ready_for_load(false));
        assert!(!session.gate_ready_for_load(true));
    }

    #[cfg(target_os = "android")]
    #[test]
    fn android_gate_passes_surface_ready() {
        use crate::playback::overlay::AndroidOverlaySession;
        let session = AndroidOverlaySession::new();
        assert!(session.gate_ready_for_load(true));
        assert!(!session.gate_ready_for_load(false));
    }

    #[cfg(target_os = "ios")]
    #[test]
    fn ios_gate_always_false() {
        use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};

        use crate::playback::overlay::IosOverlaySession;
        let session = IosOverlaySession::new(
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicBool::new(true)),
            Arc::new(AtomicU64::new(0)),
        );
        assert!(!session.gate_ready_for_load(true));
        assert!(!session.gate_ready_for_load(false));
    }
}
