//! Overlay module unit tests (mock backends + VideoSurface integration).

use parking_lot::Mutex;
use std::sync::Arc;

use crate::playback::overlay::VideoOverlayBackend;
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
fn video_surface_delegates_to_overlay_trait() {
    let surface = VideoSurface::new(Arc::new(Mutex::new(None)));
    assert!(!surface.overlay_ready_for_preroll());
    surface.cache_handle(99);
    assert!(surface.overlay_ready_for_preroll());
}
