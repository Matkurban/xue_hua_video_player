//! GStreamer 进程引导与专用 `xhvp-gst` 运行时线程。
//!
//! GStreamer process bootstrap and dedicated `xhvp-gst` runtime thread.
//!
//! 本模块负责在播放引擎使用 GStreamer 之前完成一次性初始化：平台环境变量、
//! TLS 后端、静态插件注册（iOS）、以及拥有 `GMainContext` 的专用线程。
//! 所有管线操作必须通过 [`runtime`] 中的调度 API 在该线程上执行。
//!
//! This module performs one-time setup before the playback engine uses GStreamer:
//! platform environment variables, TLS backends, static plugin registration (iOS),
//! and a dedicated thread that owns a `GMainContext`. All pipeline operations must
//! be scheduled on that thread via the [`runtime`] dispatch APIs.

mod android;
mod env;
mod init;
#[cfg(target_os = "ios")]
mod ios_plugins;
mod runtime;
mod tls;
#[cfg(target_os = "macos")]
mod tls_macos;

#[cfg(target_os = "android")]
pub use android::ensure_gst_init_android;
#[cfg(target_os = "android")]
pub use android::ensure_java_gstreamer_for_network;
pub use init::ensure_gst_init;
pub use runtime::{
    ensure_gst_runtime, gst_main_context, run_on_gst_thread, spawn_on_gst_thread,
    spawn_on_gst_thread_and_wait,
};
