//! Android overlay 子模块 — 操作函数与会话实现 / Android overlay submodule — ops and session impl.
//!
//! 重新导出 [`AndroidOverlaySession`]、native window 缓存与 Gst 刷新辅助函数。
//!
//! Re-exports [`AndroidOverlaySession`], native window caching, and Gst refresh helpers.

mod ops;
mod session;

pub use ops::{
    android_pause_preroll_with_refresh, cache_android_native_window, refresh_mobile_overlay_on_gst,
};
pub use session::{default_scheduler, AndroidOverlaySession};
