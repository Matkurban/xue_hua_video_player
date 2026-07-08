//! Shared bind-path preroll loop — decision via [`super::preroll_gate`], side effects via [`PrerollEffects`].

use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;

use super::gate::{decide_preroll_action, PipelineSnapshot, PrerollAction};
use crate::playback::shell::PipelineShell;

const MAX_BIND_PREROLL_STEPS: usize = 4;

/// Outcome of a [`PrerollEffects::resume_playing`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrerollResumeOutcome {
    /// Loop may continue (e.g. after pause preroll).
    Continue,
    /// Resume path finished; stop iterating.
    Finished,
}

/// Platform side effects for the bind-path preroll loop (mockable in unit tests).
///
/// Effects receive the shared `Arc<Mutex<PipelineShell>>` (not a held guard) so
/// they lock only in short scopes. The resume path calls the free
/// [`crate::playback::play_resume::resume_playing`], which re-locks `shell`
/// itself and must never be invoked while the lock is held (self-deadlock).
pub trait PrerollEffects {
    fn pause_preroll(
        &mut self,
        shell: &Arc<Mutex<PipelineShell>>,
        snapshot: PipelineSnapshot,
    ) -> Result<()>;

    fn resume_playing(
        &mut self,
        shell: &Arc<Mutex<PipelineShell>>,
        snapshot: PipelineSnapshot,
    ) -> Result<PrerollResumeOutcome>;
}

/// Runs the overlay-gated Ready→Paused→Playing loop for **bind** paths (`want_play` may be true).
///
/// The shell lock is never held across an effect call: each iteration takes the
/// snapshot in a short lock scope, releases it, then invokes the effect.
pub fn run_bind_preroll_loop<E: PrerollEffects>(
    shell: &Arc<Mutex<PipelineShell>>,
    want_play: bool,
    overlay_ready: bool,
    effects: &mut E,
) -> Result<()> {
    for _ in 0..MAX_BIND_PREROLL_STEPS {
        let snapshot = shell.lock().snapshot();
        let action = decide_preroll_action(snapshot, want_play, overlay_ready);
        match action {
            PrerollAction::Noop | PrerollAction::Defer => break,
            PrerollAction::PausePreroll => {
                effects.pause_preroll(shell, snapshot)?;
            }
            PrerollAction::ResumePlaying => {
                if effects.resume_playing(shell, snapshot)? == PrerollResumeOutcome::Finished {
                    break;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub struct RecordingPrerollEffects {
    pub actions: Vec<PrerollAction>,
    pub pause_count: u32,
    pub resume_count: u32,
}

#[cfg(test)]
impl RecordingPrerollEffects {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            pause_count: 0,
            resume_count: 0,
        }
    }
}

#[cfg(test)]
impl PrerollEffects for RecordingPrerollEffects {
    fn pause_preroll(
        &mut self,
        _shell: &Arc<Mutex<PipelineShell>>,
        snapshot: PipelineSnapshot,
    ) -> Result<()> {
        self.actions.push(PrerollAction::PausePreroll);
        self.pause_count += 1;
        let _ = snapshot;
        Ok(())
    }

    fn resume_playing(
        &mut self,
        _shell: &Arc<Mutex<PipelineShell>>,
        snapshot: PipelineSnapshot,
    ) -> Result<PrerollResumeOutcome> {
        self.actions.push(PrerollAction::ResumePlaying);
        self.resume_count += 1;
        let _ = snapshot;
        Ok(PrerollResumeOutcome::Finished)
    }
}
