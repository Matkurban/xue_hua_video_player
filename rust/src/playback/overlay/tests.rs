//! Overlay module unit tests (mock backends + VideoSurface integration).

use parking_lot::Mutex;
use std::sync::Arc;

use crate::playback::overlay::overlay_session::fake::FakeOverlaySession;
use crate::playback::overlay::{OverlaySession, VideoOverlayBackend};
use crate::playback::surface::VideoSurface;

struct MockOverlayState {
    stored: Arc<Mutex<Option<usize>>>,
    bound: bool,
}

impl VideoOverlayBackend for MockOverlayState {
    fn stored_handle(&self) -> &Mutex<Option<usize>> {
        self.stored.as_ref()
    }

    fn overlay_ready_for_preroll(&self) -> bool {
        self.has_cached_handle() && self.bound
    }
}

#[test]
fn mock_backend_preroll_requires_bind() {
    let stored = Arc::new(Mutex::new(Some(1usize)));
    let mock = MockOverlayState {
        stored: stored.clone(),
        bound: false,
    };
    assert!(mock.has_cached_handle());
    assert!(!mock.overlay_ready_for_preroll());

    let mock_bound = MockOverlayState {
        stored,
        bound: true,
    };
    assert!(mock_bound.overlay_ready_for_preroll());
}

#[test]
fn video_surface_delegates_overlay_ready_to_session() {
    let surface = VideoSurface::new(Arc::new(Mutex::new(None)));
    assert!(!surface.overlay_ready_for_preroll());
    surface.cache_handle(99);
    assert!(surface.overlay_ready_for_preroll());
}

#[test]
fn fake_overlay_session_gate_and_preroll() {
    use std::sync::atomic::Ordering;

    let session = FakeOverlaySession::new(true, true);
    assert!(session.gate_ready_for_load(true));
    assert!(!session.overlay_ready_for_preroll(true));
    session.bound.store(true, Ordering::SeqCst);
    assert!(session.overlay_ready_for_preroll(true));
}
