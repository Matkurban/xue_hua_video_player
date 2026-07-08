//! xhvp-gst 视频 sink 与元数据原语 / xhvp-gst video sink and metadata primitives.
//!
//! 本模块为 [`crate::playback::uri_pipeline`] 与 [`crate::playback::asset_pipeline`]
//! 提供平台视频 sink 工厂、VideoOverlay 绑定、解码元数据提取及 playbin 画面旋转。
//! 位于 Dart → [`PlaybackEngine`] → pipeline shell 链路中的 GStreamer 适配层。
//!
//! Platform video sink factory, VideoOverlay binding, decoded metadata extraction,
//! and playbin orientation for [`crate::playback::uri_pipeline`] and
//! [`crate::playback::asset_pipeline`]. GStreamer adapter layer in the
//! Dart → [`PlaybackEngine`] → pipeline shell path.

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
