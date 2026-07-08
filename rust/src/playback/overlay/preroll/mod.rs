//! 共享预卷门控与绑定路径执行器 / Shared preroll gating and bind-path executor.
//!
//! [`gate`] 提供纯函数决策；[`executor`] 在绑定路径上循环执行 Ready→Paused→Playing 转换。
//!
//! [`gate`] supplies pure decision logic; [`executor`] runs the Ready→Paused→Playing loop on bind paths.

mod executor;
mod gate;

#[cfg(test)]
pub use executor::RecordingPrerollEffects;
pub use executor::{run_bind_preroll_loop, PrerollEffects, PrerollResumeOutcome};
pub use gate::{decide_preroll_action, PipelineSnapshot, PrerollAction};
