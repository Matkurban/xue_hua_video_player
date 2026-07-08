//! xue_hua_video_player Rust crate 根模块 / Root of the xue_hua_video_player Rust crate.
//!
//! 本 crate 通过 [`flutter_rust_bridge`](https://github.com/fzyzcjy/flutter_rust_bridge) 向 Dart 暴露
//! GStreamer 播放能力。请求链路：Dart FRB → `api::player` → `PlaybackEngine`
//! → `xhvp-gst` 线程上的 pipeline / bus / overlay。
//!
//! This crate exposes GStreamer playback to Dart via FRB. Request flow:
//! Dart FRB → `api::player` → `PlaybackEngine` → pipeline / bus / overlay on `xhvp-gst`.
//!
//! # 模块树 / Module tree
//!
//! - [`api`] — FRB 边界：类型 DTO 与播放器控制入口 / FRB seam: DTOs and player control entry points
//! - [`gst`] — 进程级 GStreamer 运行时（`xhvp-gst` 线程）/ process-wide GStreamer runtime
//! - [`media`] — 媒体源解析（URI / Flutter asset）/ media source resolution
//! - [`platform`] — 平台原生桥（JNI、CALayer、Texture 等）/ native platform bridges
//! - [`playback`] — 播放引擎、pipeline、bus 归约、overlay / playback engine, pipeline, bus, overlay
//! - [`diag`] — 诊断日志（Android logcat 等）/ diagnostic logging

pub mod api;
mod frb_generated;
mod gst;
mod media;
mod platform;
mod playback;

pub(crate) mod diag;

/// FRB 生成代码引用 `crate::player_events`；在此保留别名以兼容生成层。
/// FRB-generated code references `crate::player_events`; keep this alias at the seam.
pub(crate) use api::types as player_events;
