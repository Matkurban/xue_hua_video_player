//! Linux / Windows / macOS overlay 子模块 / Linux / Windows / macOS overlay submodule.
//!
//! 导出桌面 [`OverlaySession`] 与 backend 辅助函数。macOS 播放经 Texture，Win/Linux 可选 popup overlay。
//!
//! Exports desktop [`OverlaySession`] and backend helpers. macOS uses Texture; Win/Linux may use popup overlay.

mod backend;
mod session;

#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
pub use backend::{apply_overlay_handle, DesktopOverlayBackend};
#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
pub use session::DesktopOverlaySession;
