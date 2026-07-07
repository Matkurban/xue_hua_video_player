//! Platform overlay backends — structural split from [`super::surface::VideoSurface`].

#[cfg(target_os = "ios")]
pub(crate) mod ios_session;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::{
    cache_android_native_window, refresh_mobile_overlay_on_gst, schedule_mobile_overlay_apply,
    AndroidOverlayBackend,
};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacosOverlayBackend;

#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
mod desktop;
#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
pub use desktop::{apply_overlay_handle, DesktopOverlayBackend};

#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "ios")]
pub use ios::IosLayerBackend;
#[cfg(target_os = "ios")]
pub use ios_session::{IosIdleWork, IosOverlaySession};

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod sink_slot;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use sink_slot::assign_overlay_sink;

mod preroll_gate;
pub use preroll_gate::{decide_preroll_action, PipelineSnapshot, PrerollAction};

mod video_overlay;
pub use video_overlay::VideoOverlayBackend;

#[cfg(test)]
mod tests;
