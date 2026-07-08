//! 共享绑定路径预卷循环 — 决策经 [`super::gate`]，副作用经 [`PrerollEffects`]。
//!
//! [`run_bind_preroll_loop`] 在短锁作用域内取快照，跨 effect 调用不持有 `PipelineShell` 互斥锁，
//! 避免与 [`crate::playback::play_resume::resume_playing`] 自死锁。
//!
//! Shared bind-path preroll loop — decision via [`super::gate`], side effects via [`PrerollEffects`].
//!
//! [`run_bind_preroll_loop`] snapshots under a short lock scope and never holds the
//! `PipelineShell` mutex across effect calls, avoiding self-deadlock with
//! [`crate::playback::play_resume::resume_playing`].

use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;

use super::gate::{decide_preroll_action, PipelineSnapshot, PrerollAction};
use crate::playback::shell::PipelineShell;

const MAX_BIND_PREROLL_STEPS: usize = 4;

/// [`PrerollEffects::resume_playing`] 调用的结果 / Outcome of a [`PrerollEffects::resume_playing`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrerollResumeOutcome {
    /// 循环可继续（例如暂停预卷之后）/ Loop may continue (e.g. after pause preroll).
    Continue,
    /// 恢复路径已完成；停止迭代 / Resume path finished; stop iterating.
    Finished,
}

/// 绑定路径预卷循环的平台副作用（单元测试可模拟）/ Platform side effects for the bind-path preroll loop (mockable in unit tests).
///
/// Effect 接收共享的 `Arc<Mutex<PipelineShell>>`（而非长期持有的 guard），以便在短作用域内加锁。
/// 恢复路径调用独立的 [`crate::playback::play_resume::resume_playing`]，其会重新锁定 `shell`，
/// 绝不能在已持有锁时调用（自死锁）。
///
/// Effects receive the shared `Arc<Mutex<PipelineShell>>` (not a held guard) so
/// they lock only in short scopes. The resume path calls the free
/// [`crate::playback::play_resume::resume_playing`], which re-locks `shell`
/// itself and must never be invoked while the lock is held (self-deadlock).
pub trait PrerollEffects {
    /// 执行 Paused 预卷副作用 / Executes Paused preroll side effects.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 管线壳层共享引用 / shared pipeline shell reference
    /// - `snapshot` — 决策时的管线快照 / pipeline snapshot at decision time
    ///
    /// # 返回值 / Returns
    /// - `Ok(())` 暂停成功 / `Ok(())` on successful pause
    fn pause_preroll(
        &mut self,
        shell: &Arc<Mutex<PipelineShell>>,
        snapshot: PipelineSnapshot,
    ) -> Result<()>;

    /// 执行恢复播放副作用 / Executes resume-playback side effects.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 管线壳层共享引用 / shared pipeline shell reference
    /// - `snapshot` — 决策时的管线快照 / pipeline snapshot at decision time
    ///
    /// # 返回值 / Returns
    /// - [`PrerollResumeOutcome::Continue`] 或 [`PrerollResumeOutcome::Finished`] /
    ///   [`PrerollResumeOutcome::Continue`] or [`PrerollResumeOutcome::Finished`]
    fn resume_playing(
        &mut self,
        shell: &Arc<Mutex<PipelineShell>>,
        snapshot: PipelineSnapshot,
    ) -> Result<PrerollResumeOutcome>;
}

/// 为**绑定**路径运行 overlay 门控的 Ready→Paused→Playing 循环（`want_play` 可为 `true`）/
/// Runs the overlay-gated Ready→Paused→Playing loop for **bind** paths (`want_play` may be true).
///
/// 绝不在 effect 调用期间持有 shell 锁：每轮迭代在短锁作用域内取快照，释放锁后再调用 effect。
///
/// The shell lock is never held across an effect call: each iteration takes the
/// snapshot in a short lock scope, releases it, then invokes the effect.
///
/// # 参数 / Parameters
/// - `shell` — 管线壳层 / pipeline shell
/// - `want_play` — 用户是否意图播放 / whether the user wants playback
/// - `overlay_ready` — overlay 是否就绪 / whether overlay is ready
/// - `effects` — 平台副作用实现 / platform side-effect implementation
///
/// # 返回值 / Returns
/// - `Ok(())` 循环正常结束或达到步数上限 / `Ok(())` when the loop ends normally or hits step cap
///
/// # 错误 / Errors
/// - `pause_preroll` 或 `resume_playing` 失败 / `pause_preroll` or `resume_playing` failure
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

/// 记录预卷动作的测试用 [`PrerollEffects`] 实现 / Test [`PrerollEffects`] that records preroll actions.
#[cfg(test)]
pub struct RecordingPrerollEffects {
    /// 按顺序记录的决策动作 / recorded decision actions in order
    pub actions: Vec<PrerollAction>,
    /// `pause_preroll` 调用次数 / number of `pause_preroll` invocations
    pub pause_count: u32,
    /// `resume_playing` 调用次数 / number of `resume_playing` invocations
    pub resume_count: u32,
}

#[cfg(test)]
impl RecordingPrerollEffects {
    /// 创建空记录器 / Creates an empty recorder.
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
