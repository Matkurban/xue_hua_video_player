//! 播放引擎模块根 / Playback engine module root.
//!
//! Dart FRB 层（[`crate::api::player`]）将控制命令路由到 [`PlaybackEngine`]；
//! 引擎在专用 GStreamer 线程上持有 [`gst_context::PlaybackGstContext`]（pipeline shell +
//! VideoSurface + 重放上下文），由 [`shell::PipelineShell`] 封装 `playbin3` 或 AppSrc 管线，
//! 经 [`gst`] 平台 sink 或 [`frame::FrameSink`] appsink 输出画面，[`bus`] 将 Gst 事件转为
//! Dart [`PlayerEvent`]。
//!
//! Dart FRB layer ([`crate::api::player`]) routes control commands to [`PlaybackEngine`];
//! the engine owns [`gst_context::PlaybackGstContext`] (pipeline shell + VideoSurface +
//! replay context) on a dedicated GStreamer thread. [`shell::PipelineShell`] wraps
//! `playbin3` or AppSrc pipelines; video reaches Dart via [`gst`] platform sinks or
//! [`frame::FrameSink`] appsink; [`bus`] converts Gst events into Dart [`PlayerEvent`]s.

mod asset_pipeline;
mod bus;
pub mod capabilities;
pub mod engine;
pub(crate) mod frame;
pub(crate) mod gst;
mod gst_context;
mod overlay;
mod play_resume;
mod replay;
pub(crate) mod shell;
mod sink;
mod surface;
mod switch;
mod tracks;
mod uri_pipeline;

pub use engine::{GstPlayer, PlaybackEngine};
