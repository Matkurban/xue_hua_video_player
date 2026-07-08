//! Shared preroll gating and bind-path executor.

mod executor;
mod gate;

#[cfg(test)]
pub use executor::RecordingPrerollEffects;
pub use executor::{run_bind_preroll_loop, PrerollEffects, PrerollResumeOutcome};
pub use gate::{decide_preroll_action, PipelineSnapshot, PrerollAction};
