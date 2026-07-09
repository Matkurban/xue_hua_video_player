//! 统一播放 / EOS 恢复入口 / Unified play / EOS resume — single interface for all platforms.
//!
//! [`resume_playing`] 是 [`crate::playback::engine::PlaybackEngine`] `play`/`load(autoPlay)`
//! 与 overlay 绑定完成后的唯一恢复路径；根据 overlay 就绪、EOS 与 shell 类型决定
//! 直接 Playing、seek 到起点或重建 AppSrc shell。
//!
//! [`resume_playing`] is the sole resume path for [`crate::playback::engine::PlaybackEngine`]
//! `play`/`load(autoPlay)` and post-overlay-bind; chooses direct Playing, seek-to-start,
//! or AppSrc shell rebuild based on overlay readiness, EOS, and shell kind.

use std::sync::{atomic::Ordering, Arc};

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::replay::{replay_asset_shell, PlayReplayContext};
use crate::playback::shell::{PipelineShell, SourceKind};
use crate::playback::surface::VideoSurface;
use crate::playback::switch::PipelineSwapConfig;

#[cfg(target_os = "android")]
use crate::playback::overlay::refresh_mobile_overlay_on_gst;

/// 播放/EOS 恢复的规划动作（纯决策，供测试）/ Planned pipeline action for play/EOS resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResumeAction {
    DeferOverlay,
    SetPlaying,
    SeekToStartAndPlay,
    ReplayAssetShell,
}

/// 将 overlay 就绪、EOS 与 shell 类型映射为恢复动作 / Maps overlay readiness, EOS, and shell kind to resume action.
///
/// # 参数 / Parameters
/// - `overlay_ready` — 表面是否可播放 / whether surface is ready for play
/// - `at_eos` — 是否处于 EOS / whether at end-of-stream
/// - `kind` — URI 或 Asset shell / URI or Asset shell
///
/// # 返回值 / Returns
/// - [`ResumeAction`] / planned action
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - 纯函数 / Pure function
///
/// # 平台 / Platform
/// - 与平台无关（`overlay_ready` 由调用方按平台计算）/ platform-agnostic
pub(crate) fn plan_resume_action(
    overlay_ready: bool,
    at_eos: bool,
    kind: SourceKind,
) -> ResumeAction {
    if !overlay_ready {
        return ResumeAction::DeferOverlay;
    }
    if !at_eos {
        return ResumeAction::SetPlaying;
    }
    match kind {
        SourceKind::Uri => ResumeAction::SeekToStartAndPlay,
        SourceKind::Asset => ResumeAction::ReplayAssetShell,
    }
}

/// 恢复播放或从 EOS 重放——全平台统一入口 / Resumes playback or replays from EOS — all platforms, single entry.
///
/// **必须在未持有 `shell` 锁时调用**：本函数内部会短暂加锁；若调用方已持锁
///（例如 `PlaybackEngine::run_on_gst`）会导致 `parking_lot` 不可重入自死锁并冻结 Gst MainLoop。
///
/// Must be called WITHOUT the `shell` mutex held; self-deadlock if called from `run_on_gst`.
///
/// # 参数 / Parameters
/// - `shell` — 共享 pipeline shell 锁 / shared pipeline shell lock
/// - `replay` — 重放上下文 / replay context
/// - `swap` — shell 重建元数据 / shell rebuild metadata
/// - `surface` — VideoSurface / video surface
/// - `overlay_ready` — 是否可立即播放 / whether overlay is ready
///
/// # 返回值 / Returns
/// - 成功：`Ok(())`；overlay 未就绪时延迟（仍返回 Ok）/ `Ok(())`; defers when overlay not ready
///
/// # 错误 / Errors
/// - 状态切换、seek 或 asset 重放失败 / state change, seek, or asset replay failure
///
/// # 线程 / Threading
/// - 必须在 Gst 线程上调用；内部自行加锁 shell / Gst thread; locks shell internally
///
/// # 平台 / Platform
/// - Android：Playing 后刷新 mobile overlay / refreshes mobile overlay after Playing on Android
pub fn resume_playing(
    shell: Arc<Mutex<PipelineShell>>,
    replay: &PlayReplayContext,
    swap: &PipelineSwapConfig,
    surface: &VideoSurface,
    overlay_ready: bool,
) -> Result<()> {
    // Guard the calling convention: try_lock returns None when the current
    // thread already holds `shell`, which would otherwise deadlock below.
    debug_assert!(
        shell.try_lock().is_some(),
        "resume_playing called with the shell lock already held (would self-deadlock)"
    );
    let kind = {
        let guard = shell.lock();
        guard.source_kind()
    };

    let at_eos = replay.at_eos.load(Ordering::SeqCst);
    let action = plan_resume_action(overlay_ready, at_eos, kind);

    if action == ResumeAction::DeferOverlay {
        log::info!("gst: deferring play until overlay is ready");
        return Ok(());
    }

    if at_eos {
        replay.at_eos.store(false, Ordering::SeqCst);
    }

    match action {
        ResumeAction::DeferOverlay => unreachable!(),
        ResumeAction::SetPlaying => {
            let guard = shell.lock();
            guard.set_state_sync(gst::State::Playing)?;
        }
        ResumeAction::SeekToStartAndPlay => {
            // Manual replay after EOS resets speed to 1.0 (looping keeps the
            // speed via EosLoopSeek, which is a different path).
            *replay.rate.lock() = 1.0;
            let guard = shell.lock();
            guard.seek_to_start_with_rate(1.0)?;
            guard.set_state_sync(gst::State::Playing)?;
        }
        ResumeAction::ReplayAssetShell => {
            // Manual asset replay after EOS resets speed to 1.0; the rebuilt
            // shell already starts at rate 1.0.
            *replay.rate.lock() = 1.0;
            let mut guard = shell.lock();
            replay_asset_shell(&mut guard, replay, swap, surface)?;
        }
    }

    #[cfg(target_os = "android")]
    android_refresh_after_playing(&shell, surface)?;

    Ok(())
}

#[cfg(target_os = "android")]
fn android_refresh_after_playing(
    shell: &Arc<Mutex<PipelineShell>>,
    surface: &VideoSurface,
) -> Result<()> {
    if let Some(handle) = *surface.stored_handle().lock() {
        let (width, height) = surface.cached_dimensions();
        let guard = shell.lock();
        refresh_mobile_overlay_on_gst(&guard, handle, width, height, "after Playing")?;
    }
    Ok(())
}

/// overlay 绑定完成后，若用户已请求播放则恢复 / After overlay bind, resume if user already requested play.
///
/// 绑定路径必须调用本函数（或 `overlay_ready: true` 的 [`resume_playing`]），以便在
/// `desired_playing` 先于原生 surface 设置时各平台行为一致。若 load 延迟了 Paused preroll，
/// 管线可能仍在 Ready——先转 Paused 再 Playing 以完成 prepare-window-handle。
///
/// # 参数 / Parameters
/// - `shell`、`replay`、`swap`、`surface` — 同 [`resume_playing`] / same as [`resume_playing`]
///
/// # 返回值 / Returns
/// - 成功：`Ok(())`；`desired_playing=false` 时无操作 / `Ok(())`; no-op when not desired
///
/// # 错误 / Errors
/// - preroll 或 [`resume_playing`] 失败 / preroll or resume failure
///
/// # 线程 / Threading
/// - Gst 线程 / Gst thread
///
/// # 平台 / Platform
/// - 主要用于需原生 overlay 绑定的平台 / platforms requiring native overlay bind
pub fn maybe_resume_after_overlay_bind(
    shell: Arc<Mutex<PipelineShell>>,
    replay: &PlayReplayContext,
    swap: &PipelineSwapConfig,
    surface: &VideoSurface,
) -> Result<()> {
    if !replay.desired_playing.load(Ordering::SeqCst) {
        log::debug!("gst: overlay bound — desired_playing=false, skip resume");
        return Ok(());
    }
    {
        let guard = shell.lock();
        let snap = guard.snapshot();
        if snap.current == gst::State::Ready
            && snap.pending == gst::State::VoidPending
            && snap.has_pending_media
        {
            log::info!("gst: overlay bound — PAUSED preroll before play");
            guard.set_state_sync(gst::State::Paused)?;
        }
    }
    log::info!("gst: overlay bound — resuming play (desired_playing=true)");
    resume_playing(shell, replay, swap, surface, true)
}

/// 纯门控：绑定完成后是否应调用 [`resume_playing`] / Pure gate for post-bind resume.
pub(crate) fn should_resume_after_overlay_bind(desired_playing: bool) -> bool {
    desired_playing
}

/// 计算当前平台上 surface 是否可用于播放恢复 / Whether the surface is ready for play resume on the active platform.
///
/// Apple（iOS/macOS）与桌面（Windows/Linux）经 Flutter 外部纹理（appsink）渲染，无原生
/// overlay 绑定需求，恒为 ready。Android 等需 GStreamer 侧真实绑定。
///
/// # 参数 / Parameters
/// - `surface` — VideoSurface / video surface
///
/// # 返回值 / Returns
/// - `true` 表示可立即 resume / `true` if resume can proceed
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - 任意线程 / any thread
///
/// # 平台 / Platform
/// - 纹理平台恒 `true`；Android 需 `is_overlay_bound_on_gst` / texture platforms always true
pub fn overlay_ready_for_play(surface: &VideoSurface) -> bool {
    #[cfg(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    ))]
    {
        let _ = surface;
        true
    }
    #[cfg(not(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    )))]
    {
        surface.is_overlay_bound_on_gst()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_table_overlay_not_ready_defers() {
        assert_eq!(
            plan_resume_action(false, false, SourceKind::Uri),
            ResumeAction::DeferOverlay
        );
        assert_eq!(
            plan_resume_action(false, true, SourceKind::Asset),
            ResumeAction::DeferOverlay
        );
    }

    #[test]
    fn gate_table_ready_not_eos_plays() {
        assert_eq!(
            plan_resume_action(true, false, SourceKind::Uri),
            ResumeAction::SetPlaying
        );
        assert_eq!(
            plan_resume_action(true, false, SourceKind::Asset),
            ResumeAction::SetPlaying
        );
    }

    #[test]
    fn gate_table_eos_uri_seeks() {
        assert_eq!(
            plan_resume_action(true, true, SourceKind::Uri),
            ResumeAction::SeekToStartAndPlay
        );
    }

    #[test]
    fn gate_table_eos_asset_replays_shell() {
        assert_eq!(
            plan_resume_action(true, true, SourceKind::Asset),
            ResumeAction::ReplayAssetShell
        );
    }

    #[test]
    fn should_resume_after_bind_follows_desired_playing() {
        assert!(!should_resume_after_overlay_bind(false));
        assert!(should_resume_after_overlay_bind(true));
    }

    // Android binds a native VideoOverlay, so readiness requires a real
    // GStreamer bind (a cached handle alone is not enough).
    #[cfg(not(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    )))]
    #[test]
    fn overlay_ready_for_play_requires_bound_handle() {
        use parking_lot::Mutex;

        let surface = VideoSurface::new(Arc::new(Mutex::new(None)));
        assert!(!overlay_ready_for_play(&surface));
        surface.cache_handle(0x1000);
        // Cache alone is not a GStreamer bind on any platform; bind flags stay false.
        assert!(!overlay_ready_for_play(&surface));
    }

    // Texture platforms render through a Flutter external texture (appsink).
    #[cfg(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    ))]
    #[test]
    fn overlay_ready_for_play_texture_always_ready() {
        use parking_lot::Mutex;

        let surface = VideoSurface::new(Arc::new(Mutex::new(None)));
        assert!(overlay_ready_for_play(&surface));
    }

    // Regression: `PlaybackEngine::run_on_gst` pre-locks the shell, and calling
    // `pipeline_play` -> `resume_playing` from inside it re-locked the same
    // non-reentrant mutex, self-deadlocking the gst MainLoop (2nd open / resume
    // after pause hung). `resume_playing` must be called without the shell lock;
    // this verifies the guard rejects a caller-held lock instead of deadlocking.
    #[test]
    fn resume_playing_rejects_caller_held_shell_lock() {
        use crate::playback::bus::Emitter;
        use crate::playback::gst::{InternalAspectRatioMode, InternalVideoMetadata};
        use crate::playback::shell::new_test_shell;
        use crate::playback::tracks::TrackCache;
        use gstreamer as gst;
        use std::sync::atomic::AtomicBool;

        let _ = gst::init();
        let shell = Arc::new(Mutex::new(new_test_shell(
            gst::Pipeline::new(),
            gst::ElementFactory::make("fakesink")
                .build()
                .expect("fakesink"),
            SourceKind::Uri,
            None,
        )));
        let replay = PlayReplayContext {
            desired_playing: Arc::new(AtomicBool::new(true)),
            at_eos: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(true)),
            rate: Arc::new(Mutex::new(1.0)),
        };
        let swap = PipelineSwapConfig {
            emitter: Arc::new(Mutex::new(None::<Emitter>)),
            looping: Arc::new(AtomicBool::new(false)),
            metadata: Arc::new(Mutex::new(InternalVideoMetadata::default())),
            track_cache: Arc::new(Mutex::new(TrackCache::default())),
            rotate_degrees: 0,
            aspect: InternalAspectRatioMode::default(),
            frame_sink: crate::playback::frame::FrameSink::new(),
        };
        let surface = VideoSurface::new(Arc::new(Mutex::new(None)));

        let held = shell.lock();
        let shell_for_call = shell.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = resume_playing(shell_for_call, &replay, &swap, &surface, true);
        }));
        drop(held);
        assert!(
            result.is_err(),
            "resume_playing must reject a caller-held shell lock (guard), not deadlock"
        );
    }

    // Regression: manual replay after EOS (SeekToStartAndPlay) resets the shared
    // rate to 1.0 (looping keeps its rate via a different path). The UI mirrors
    // this reset in PlaybackSession.play().
    #[test]
    fn manual_eos_replay_resets_rate_to_one() {
        use crate::playback::bus::Emitter;
        use crate::playback::gst::{InternalAspectRatioMode, InternalVideoMetadata};
        use crate::playback::shell::new_test_shell;
        use crate::playback::tracks::TrackCache;
        use gstreamer as gst;
        use gstreamer::prelude::*;
        use std::sync::atomic::AtomicBool;

        let _ = gst::init();
        let pipeline = gst::Pipeline::new();
        let src = gst::ElementFactory::make("audiotestsrc")
            .property("is-live", false)
            .build()
            .expect("audiotestsrc");
        let sink = gst::ElementFactory::make("fakesink")
            .property("sync", false)
            .build()
            .expect("fakesink");
        pipeline.add_many([&src, &sink]).expect("add");
        src.link(&sink).expect("link");
        let shell = Arc::new(Mutex::new(new_test_shell(
            pipeline,
            sink,
            SourceKind::Uri,
            None,
        )));
        // Preroll to PLAYING so the replay seek is valid, then release the lock
        // (resume_playing locks internally).
        shell
            .lock()
            .set_state_sync(gst::State::Playing)
            .expect("to playing");

        let replay = PlayReplayContext {
            desired_playing: Arc::new(AtomicBool::new(true)),
            at_eos: Arc::new(AtomicBool::new(true)),
            running: Arc::new(AtomicBool::new(true)),
            rate: Arc::new(Mutex::new(2.0)),
        };
        let swap = PipelineSwapConfig {
            emitter: Arc::new(Mutex::new(None::<Emitter>)),
            looping: Arc::new(AtomicBool::new(false)),
            metadata: Arc::new(Mutex::new(InternalVideoMetadata::default())),
            track_cache: Arc::new(Mutex::new(TrackCache::default())),
            rotate_degrees: 0,
            aspect: InternalAspectRatioMode::default(),
            frame_sink: crate::playback::frame::FrameSink::new(),
        };
        let surface = VideoSurface::new(Arc::new(Mutex::new(None)));

        resume_playing(shell.clone(), &replay, &swap, &surface, true).expect("resume");
        assert_eq!(
            *replay.rate.lock(),
            1.0,
            "manual EOS replay must reset rate to 1.0"
        );
        let _ = shell.lock().set_state_sync(gst::State::Null);
    }
}
