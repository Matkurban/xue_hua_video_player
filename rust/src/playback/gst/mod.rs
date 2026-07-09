//! GStreamer 视频原语：sink、元数据、旋转与宽高比 / GStreamer video primitives: sink, metadata, rotation, aspect ratio.
//!
//! 提供平台视频 sink 工厂、VideoOverlay 绑定、解码元数据提取，
//! 以及 URI playbin 与 AppSrc 管线的画面旋转。
//!
//! Provides platform video sink factory, VideoOverlay binding, decoded metadata extraction,
//! and video rotation for URI playbin and AppSrc pipelines.

mod metadata;
mod orientation;
mod sink;

pub(crate) use metadata::InternalVideoMetadata;
pub(crate) use orientation::{
    apply_rotation_to_element, make_orientation_element, make_videoflip_element, rotate_method,
    rotate_video_direction, validate_rotate_degrees, InternalAspectRatioMode,
};
pub(crate) use sink::{
    attach_overlay_bus_sync_handler, build_video_sink_bin, clear_overlay_window_handle,
    create_platform_video_sink, expose_overlay, set_overlay_render_rectangle,
    set_overlay_window_handle,
};
#[cfg(target_os = "android")]
pub(crate) use sink::link_android_gl_video_branch;
