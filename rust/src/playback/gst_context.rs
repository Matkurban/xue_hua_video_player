//! Gst 线程上下文捆绑 / Bundled Gst-thread context for load, play, switch, and overlay paths.
//!
//! [`PlaybackGstContext`] 在 [`crate::playback::engine::PlaybackEngine`] 与 Gst 线程闭包之间
//! 共享 pipeline shell、VideoSurface、重放原子量及 swap 配置；[`PlaybackGstAsyncSnapshot`]
//! 为 `spawn_on_gst_thread_and_wait` 传递的异步安全快照。
//!
//! [`PlaybackGstContext`] shares pipeline shell, VideoSurface, replay atomics, and swap config
//! between [`crate::playback::engine::PlaybackEngine`] and Gst-thread closures;
//! [`PlaybackGstAsyncSnapshot`] is the async-safe snapshot passed to
//! `spawn_on_gst_thread_and_wait`.

use std::sync::{atomic::AtomicBool, Arc};

use parking_lot::Mutex;

use crate::playback::bus::Emitter;
use crate::playback::frame::FrameSink;
use crate::playback::gst::InternalVideoMetadata;
use crate::playback::gst::{InternalAspectRatioMode, InternalVideoOrientationConfig};
use crate::playback::replay::{OverlayPlayIntent, PlayReplayContext};
use crate::playback::shell::PipelineShell;
#[cfg(target_os = "android")]
use crate::playback::sink::OverlaySizeSync;
use crate::playback::surface::VideoSurface;
use crate::playback::switch::PipelineSwapConfig;
use crate::playback::tracks::TrackCache;

/// 引擎持有的实时 Gst 捆绑体；`shell` 与 `surface` 为权威状态 / Live engine-owned Gst bundle — `shell` and `surface` are canonical.
pub struct PlaybackGstContext {
    /// 加锁的 pipeline shell（playbin 或 AppSrc）/ Locked pipeline shell (playbin or AppSrc).
    pub shell: Arc<Mutex<PipelineShell>>,
    /// 原生 overlay / 纹理表面状态 / Native overlay / texture surface state.
    pub surface: VideoSurface,
    /// 播放意图与 EOS 重放共享原子量 / Shared atomics for play intent and EOS replay.
    pub replay: PlayReplayContext,
    emitter: Arc<Mutex<Option<Emitter>>>,
    looping: Arc<AtomicBool>,
    metadata: Arc<Mutex<InternalVideoMetadata>>,
    track_cache: Arc<Mutex<TrackCache>>,
    orientation: Arc<Mutex<InternalVideoOrientationConfig>>,
    aspect_mode: Arc<Mutex<InternalAspectRatioMode>>,
    frame_sink: Arc<FrameSink>,
    #[cfg(target_os = "android")]
    overlay_size_sync: Option<OverlaySizeSync>,
}

/// 传入 Gst 线程闭包的异步安全快照 / Async-safe snapshot passed into Gst thread closures.
pub struct PlaybackGstAsyncSnapshot {
    /// Pipeline shell 共享锁 / Shared pipeline shell lock.
    pub shell: Arc<Mutex<PipelineShell>>,
    /// 可克隆的 surface 句柄（用于 switch）/ Clonable surface handle (for switch).
    pub surface: VideoSurface,
    /// 重放上下文（共享原子身份）/ Replay context (shared atomic identity).
    pub replay: PlayReplayContext,
    /// URI ↔ 资产切换时的管线元数据 / Pipeline metadata for URI ↔ asset swaps.
    pub swap: PipelineSwapConfig,
}

impl Clone for PlaybackGstAsyncSnapshot {
    fn clone(&self) -> Self {
        Self {
            shell: self.shell.clone(),
            surface: self.surface.clone_for_switch(),
            replay: self.replay.clone(),
            swap: self.swap.clone_for_async(),
        }
    }
}

impl PlaybackGstContext {
    /// 构造新的 Gst 上下文 / Constructs a new Gst context.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 已安装的 [`PipelineShell`] / installed [`PipelineShell`]
    /// - `surface` — [`VideoSurface`] overlay 状态 / [`VideoSurface`] overlay state
    /// - `replay` — 播放/重放共享原子量 / shared play/replay atomics
    /// - `emitter` — 可选事件发射器 / optional event emitter
    /// - `looping` — 循环播放标志 / looping flag
    /// - `metadata` — 视频元数据缓存 / video metadata cache
    /// - `track_cache` — 多轨缓存 / multi-track cache
    /// - `orientation` — 画面旋转配置 / orientation config
    /// - `aspect_mode` — 宽高比模式 / aspect ratio mode
    /// - `frame_sink` — Flutter 外部纹理帧源 / Flutter external texture frame source
    /// - `overlay_size_sync`（Android）— 解码尺寸变更回调 / decoded dimension change callback
    ///
    /// # 返回值 / Returns
    /// - 新的 [`PlaybackGstContext`] / new context instance
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 通常在 Gst 线程初始化后于引擎构造路径调用 / Called on engine construction after Gst init
    ///
    /// # 平台 / Platform
    /// - Android 可选 `overlay_size_sync` / optional `overlay_size_sync` on Android
    pub fn new(
        shell: Arc<Mutex<PipelineShell>>,
        surface: VideoSurface,
        replay: PlayReplayContext,
        emitter: Arc<Mutex<Option<Emitter>>>,
        looping: Arc<AtomicBool>,
        metadata: Arc<Mutex<InternalVideoMetadata>>,
        track_cache: Arc<Mutex<TrackCache>>,
        orientation: Arc<Mutex<InternalVideoOrientationConfig>>,
        aspect_mode: Arc<Mutex<InternalAspectRatioMode>>,
        frame_sink: Arc<FrameSink>,
        #[cfg(target_os = "android")] overlay_size_sync: Option<OverlaySizeSync>,
    ) -> Self {
        Self {
            shell,
            surface,
            replay,
            emitter,
            looping,
            metadata,
            track_cache,
            orientation,
            aspect_mode,
            frame_sink,
            #[cfg(target_os = "android")]
            overlay_size_sync,
        }
    }

    /// 构建 URI ↔ 资产 shell 重建用的 swap 配置 / Builds swap config for URI ↔ asset shell rebuilds.
    ///
    /// # 参数 / Parameters
    /// - 无（读取 `self` 内部锁与克隆 Arc）/ None (reads internal locks and clones Arcs)
    ///
    /// # 返回值 / Returns
    /// - [`PipelineSwapConfig`]，含当前 orientation/aspect 快照 / with current orientation/aspect snapshot
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 短暂持有 `orientation`/`aspect_mode` 锁 / briefly holds orientation/aspect locks
    ///
    /// # 平台 / Platform
    /// - Android 包含 `overlay_size_sync` / includes `overlay_size_sync` on Android
    pub fn swap_config(&self) -> PipelineSwapConfig {
        PipelineSwapConfig {
            emitter: self.emitter.clone(),
            looping: self.looping.clone(),
            metadata: self.metadata.clone(),
            track_cache: self.track_cache.clone(),
            orientation: *self.orientation.lock(),
            aspect: *self.aspect_mode.lock(),
            frame_sink: self.frame_sink.clone(),
            #[cfg(target_os = "android")]
            overlay_size_sync: self.overlay_size_sync.clone(),
        }
    }

    /// 移动端 overlay 绑定时的统一播放意图 / Unified play intent for mobile overlay bind.
    ///
    /// # 参数 / Parameters
    /// - 无 / None
    ///
    /// # 返回值 / Returns
    /// - [`OverlayPlayIntent`]，含实时 replay 与 swap 快照 / with live replay and swap snapshot
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 任意线程；内部调用 [`swap_config`] / any thread; calls [`swap_config`] internally
    ///
    /// # 平台 / Platform
    /// - 主要用于 Android/iOS overlay 绑定路径 / primarily Android/iOS overlay bind paths
    pub fn overlay_intent(&self) -> OverlayPlayIntent {
        OverlayPlayIntent {
            replay: self.replay.clone(),
            swap: self.swap_config(),
        }
    }

    /// 克隆异步安全快照供 `spawn_on_gst_thread_and_wait` 使用 / Clones async-safe snapshot for Gst thread dispatch.
    ///
    /// # 参数 / Parameters
    /// - 无 / None
    ///
    /// # 返回值 / Returns
    /// - [`PlaybackGstAsyncSnapshot`] / async snapshot
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 任意线程可调用 / callable from any thread
    ///
    /// # 平台 / Platform
    /// - 与平台无关 / Platform-independent
    pub fn clone_for_async(&self) -> PlaybackGstAsyncSnapshot {
        PlaybackGstAsyncSnapshot {
            shell: self.shell.clone(),
            surface: self.surface.clone_for_switch(),
            replay: self.replay.clone(),
            swap: self.swap_config().clone_for_async(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    use gstreamer as gst;

    use crate::playback::shell::SourceKind;

    fn sample_context_with_swap(
        orientation: InternalVideoOrientationConfig,
        aspect: InternalAspectRatioMode,
    ) -> PlaybackGstContext {
        let _ = gst::init();
        let shell = crate::playback::shell::new_test_shell(
            gst::Pipeline::new(),
            gst::ElementFactory::make("fakesink")
                .build()
                .expect("fakesink"),
            SourceKind::Uri,
            None,
        );
        let desired = Arc::new(AtomicBool::new(true));
        let at_eos = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let overlay = Arc::new(Mutex::new(None));
        let surface = VideoSurface::new(overlay);
        PlaybackGstContext::new(
            Arc::new(Mutex::new(shell)),
            surface,
            PlayReplayContext {
                desired_playing: desired,
                at_eos,
                running,
                rate: Arc::new(Mutex::new(1.0)),
            },
            Arc::new(Mutex::new(None)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(InternalVideoMetadata::default())),
            Arc::new(Mutex::new(TrackCache::default())),
            Arc::new(Mutex::new(orientation)),
            Arc::new(Mutex::new(aspect)),
            crate::playback::frame::FrameSink::new(),
            #[cfg(target_os = "android")]
            None,
        )
    }

    fn sample_context() -> PlaybackGstContext {
        sample_context_with_swap(
            InternalVideoOrientationConfig::default(),
            InternalAspectRatioMode::default(),
        )
    }

    #[test]
    fn clone_for_async_snapshots_orientation_and_aspect_from_locks() {
        let ctx = sample_context_with_swap(
            InternalVideoOrientationConfig {
                rotate_degrees: 90,
                ..Default::default()
            },
            InternalAspectRatioMode::Fill,
        );

        let snap = ctx.clone_for_async();
        assert_eq!(snap.swap.orientation.rotate_degrees, 90);
        assert_eq!(snap.swap.aspect, InternalAspectRatioMode::Fill);
    }

    #[test]
    fn clone_for_async_shares_replay_atomic_identity() {
        let ctx = sample_context();
        let snap = ctx.clone_for_async();
        assert!(Arc::ptr_eq(
            &ctx.replay.desired_playing,
            &snap.replay.desired_playing
        ));
    }

    #[test]
    fn overlay_intent_uses_live_swap_snapshot() {
        let ctx = sample_context_with_swap(
            InternalVideoOrientationConfig::default(),
            InternalAspectRatioMode::Fit,
        );
        ctx.replay.desired_playing.store(false, Ordering::SeqCst);

        let intent = ctx.overlay_intent();
        assert!(!intent.replay.desired_playing.load(Ordering::SeqCst));
        assert_eq!(intent.swap.aspect, InternalAspectRatioMode::Fit);
        assert!(Arc::ptr_eq(
            &ctx.replay.desired_playing,
            &intent.replay.desired_playing
        ));
    }
}
