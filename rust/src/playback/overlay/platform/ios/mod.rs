//! iOS overlay 子模块 — bus 后端与会话 / iOS overlay submodule — bus backend and session.
//!
//! 导出 [`IosOverlaySession`]、[`IosLayerBackend`] 与 idle 工作上下文 [`IosIdleWork`]。
//!
//! Exports [`IosOverlaySession`], [`IosLayerBackend`], and idle work context [`IosIdleWork`].

mod bus_backend;
mod session;

pub use bus_backend::IosLayerBackend;
pub use session::{IosIdleWork, IosOverlaySession};
