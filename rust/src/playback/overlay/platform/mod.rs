//! 平台 overlay 适配器（Android、iOS、桌面/macOS 窗口）/ Platform overlay adapters (Android, iOS, desktop/macOS window).
//!
//! 按编译目标条件导出各子模块的 session 与底层操作函数。
//!
//! Conditionally exports per-submodule sessions and low-level ops by compile target.

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
pub mod window;
