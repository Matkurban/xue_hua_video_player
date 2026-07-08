//! GStreamer video sink primitives and pipeline metadata.

mod metadata;
mod orientation;
mod sink;

pub(crate) use metadata::InternalVideoMetadata;
pub(crate) use orientation::{
    apply_orientation_to_playbin, InternalAspectRatioMode, InternalVideoOrientationConfig,
};
pub(crate) use sink::{
    attach_overlay_bus_sync_handler, clear_overlay_window_handle, create_platform_video_sink,
    expose_overlay, set_overlay_render_rectangle, set_overlay_window_handle,
};

#[cfg(target_os = "ios")]
pub(crate) use sink::bus_sync_reply_for_ios_overlay;
