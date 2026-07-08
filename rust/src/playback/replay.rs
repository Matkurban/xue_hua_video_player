//! 播放/重放共享上下文 / Play/replay context shared between overlay attach and EOS replay paths.
//!
//! [`PlayReplayContext`] 在 [`crate::playback::engine::PlaybackEngine`]、overlay 绑定与
//! EOS 重放路径间共享 `desired_playing`、`at_eos`、播放速率等原子状态；
//! [`OverlayPlayIntent`] 将 replay 与 [`crate::playback::switch::PipelineSwapConfig`] 打包供移动端绑定使用。
//!
//! [`PlayReplayContext`] shares `desired_playing`, `at_eos`, playback rate, and related atomics
//! across [`crate::playback::engine::PlaybackEngine`], overlay bind, and EOS replay paths;
//! [`OverlayPlayIntent`] bundles replay with [`crate::playback::switch::PipelineSwapConfig`]
//! for mobile overlay bind.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;
use crate::playback::switch::{switch_asset_shell, PipelineSwapConfig};

/// overlay 绑定与恢复路径的播放意图原子量 / Playback intent atomics for overlay bind and resume paths.
#[derive(Clone)]
pub struct PlayReplayContext {
    /// 用户是否请求播放（可能在 overlay 绑定前设置）/ Whether the user requested play (may be set before overlay bind).
    pub desired_playing: Arc<AtomicBool>,
    /// 是否处于 EOS（影响重放策略）/ Whether playback reached EOS (affects replay strategy).
    pub at_eos: Arc<AtomicBool>,
    /// 引擎是否仍在运行（Drop 时置 false）/ Whether the engine is still running (cleared on Drop).
    pub running: Arc<AtomicBool>,
    /// 当前播放速率，与引擎共享以便 EOS 重放/循环保持用户选速 / Current playback rate, shared with engine so EOS replay/loop keeps user speed.
    pub rate: Arc<Mutex<f64>>,
}

/// 移动端 overlay 绑定的统一播放意图（Android + iOS）/ Unified play intent for mobile overlay bind (Android + iOS).
#[derive(Clone)]
pub struct OverlayPlayIntent {
    /// 重放上下文 / Replay context.
    pub replay: PlayReplayContext,
    /// 管线 swap 配置 / Pipeline swap config.
    pub swap: PipelineSwapConfig,
}

impl OverlayPlayIntent {
    /// 克隆供异步 Gst 闭包使用的意图（共享原子身份）/ Clones intent for async Gst closures (shared atomic identity).
    ///
    /// # 参数 / Parameters
    /// - 无 / None
    ///
    /// # 返回值 / Returns
    /// - 新的 [`OverlayPlayIntent`]，Arc 指针与源相同 / new intent sharing Arc pointers with source
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 任意线程 / any thread
    ///
    /// # 平台 / Platform
    /// - Android、iOS overlay 绑定 / Android, iOS overlay bind
    pub fn clone_for_async(&self) -> Self {
        Self {
            replay: self.replay.clone(),
            swap: self.swap.clone_for_async(),
        }
    }
}

/// 从 EOS 重放资产：拆除并重建 AppSrc shell（全新 decodebin）/ Replays an asset from EOS by tearing down and rebuilding the shell (fresh decodebin).
///
/// # 参数 / Parameters
/// - `shell` — 可变 pipeline shell 引用 / mutable pipeline shell reference
/// - `replay` — 重放上下文（清除 `at_eos` 由调用方负责）/ replay context
/// - `swap` — shell 重建元数据 / shell rebuild metadata
/// - `surface` — VideoSurface（preroll 与 overlay 重绑）/ VideoSurface for preroll and overlay rebind
///
/// # 返回值 / Returns
/// - 成功：`Ok(())`，管线进入 Playing / `Ok(())` with pipeline in Playing
///
/// # 错误 / Errors
/// - 缺少 `asset_key` / missing `asset_key`
/// - [`switch_asset_shell`] 或 `set_state_sync(Playing)` 失败 / shell switch or state change failure
///
/// # 线程 / Threading
/// - 必须在 Gst 线程上调用，且调用方已持有 `shell` 锁 / Must run on Gst thread with caller holding `shell` lock
///
/// # 平台 / Platform
/// - 仅资产（AppSrc）管线；Android 记录诊断日志 / AppSrc pipelines only; Android logs diagnostics
pub fn replay_asset_shell(
    shell: &mut PipelineShell,
    replay: &PlayReplayContext,
    swap: &PipelineSwapConfig,
    surface: &VideoSurface,
) -> Result<()> {
    let key = shell
        .asset_key()
        .ok_or_else(|| anyhow!("asset replay requested but asset_key missing"))?
        .to_string();
    switch_asset_shell(shell, &key, swap, replay, surface)?;
    shell.set_state_sync(gst::State::Playing)?;
    #[cfg(target_os = "android")]
    crate::diag::logcat_info("gst: AppSrc replay from EOS (shell reload)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::bus::Emitter;
    use crate::playback::gst::{
        InternalAspectRatioMode, InternalVideoMetadata, InternalVideoOrientationConfig,
    };
    use crate::playback::surface::VideoSurface;
    use crate::playback::tracks::TrackCache;

    fn sample_intent() -> OverlayPlayIntent {
        let desired = Arc::new(AtomicBool::new(true));
        let at_eos = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        OverlayPlayIntent {
            replay: PlayReplayContext {
                desired_playing: desired,
                at_eos,
                running,
                rate: Arc::new(Mutex::new(1.0)),
            },
            swap: PipelineSwapConfig {
                emitter: Arc::new(Mutex::new(None)),
                looping: Arc::new(AtomicBool::new(false)),
                metadata: Arc::new(Mutex::new(InternalVideoMetadata::default())),
                track_cache: Arc::new(Mutex::new(TrackCache::default())),
                orientation: InternalVideoOrientationConfig::default(),
                aspect: InternalAspectRatioMode::default(),
                frame_sink: crate::playback::frame::FrameSink::new(),
            },
        }
    }

    #[test]
    fn intent_clone_for_async_shares_atomic_identity() {
        let intent = sample_intent();
        let cloned = intent.clone_for_async();
        assert!(Arc::ptr_eq(
            &intent.replay.desired_playing,
            &cloned.replay.desired_playing
        ));
    }
}
