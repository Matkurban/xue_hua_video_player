//! Platform overlay backends — structural split from [`super::surface::VideoSurface`].

mod gst_scheduler;
mod overlay_session;
mod platform;
mod preroll;
mod video_overlay;

#[cfg(test)]
mod tests;

pub use gst_scheduler::{GstTaskScheduler, SpawnOnGstThreadScheduler};
pub use overlay_session::OverlaySession;
pub use preroll::{
    decide_preroll_action, run_bind_preroll_loop, PipelineSnapshot, PrerollAction, PrerollEffects,
    PrerollResumeOutcome,
};
pub use video_overlay::VideoOverlayBackend;

#[cfg(target_os = "android")]
pub use platform::android::{
    cache_android_native_window, default_scheduler, refresh_mobile_overlay_on_gst,
    AndroidOverlaySession,
};

#[cfg(target_os = "ios")]
pub use platform::ios::{IosIdleWork, IosLayerBackend, IosOverlaySession};

#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
pub use platform::window::{apply_overlay_handle, DesktopOverlayBackend, DesktopOverlaySession};

#[cfg(target_os = "macos")]
pub use platform::window::{MacosOverlayBackend, MacosOverlaySession};
