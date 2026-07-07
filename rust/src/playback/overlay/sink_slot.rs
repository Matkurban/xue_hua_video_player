//! Shared overlay sink slot helpers (macOS + iOS playbin rebuild).

#[cfg(any(target_os = "macos", target_os = "ios"))]
use gstreamer as gst;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use parking_lot::Mutex;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn assign_overlay_sink(slot: &std::sync::Arc<Mutex<gst::Element>>, element: &gst::Element) {
    *slot.lock() = element.clone();
}
