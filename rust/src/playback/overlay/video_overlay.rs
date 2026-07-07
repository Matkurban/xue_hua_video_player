//! Shared overlay backend contract (used by [`super::super::surface::VideoSurface`] and tests).

use parking_lot::Mutex;

/// Minimal overlay state surface shared across platforms.
pub trait VideoOverlayBackend {
    fn stored_handle(&self) -> &Mutex<Option<usize>>;

    fn cache_handle(&self, handle: usize) {
        if handle == 0 {
            self.stored_handle().lock().take();
        } else {
            *self.stored_handle().lock() = Some(handle);
        }
    }

    fn has_cached_handle(&self) -> bool {
        self.stored_handle().lock().is_some()
    }

    /// True when preroll may proceed (platform-specific bind rules).
    fn overlay_ready_for_preroll(&self) -> bool {
        self.has_cached_handle()
    }
}
