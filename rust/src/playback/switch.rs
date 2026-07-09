//! URI ↔ 资产管线 shell 切换 / URI ↔ asset pipeline shell switching.
//!
//! [`switch_shell`] 在 [`crate::playback::engine::PlaybackEngine::load`] 时根据
//! [`crate::media::ResolvedSource`] 重建或重配置 [`crate::playback::shell::PipelineShell`]，
//! 并应用 overlay 重绑、宽高比与 orientation；[`PipelineSwapConfig`] 携带跨 shell 重建的元数据。
//!
//! [`switch_shell`] rebuilds or reconfigures [`crate::playback::shell::PipelineShell`] during
//! [`crate::playback::engine::PlaybackEngine::load`] based on [`crate::media::ResolvedSource`],
//! applying overlay rebind, aspect ratio, and orientation; [`PipelineSwapConfig`] carries
//! metadata across shell rebuilds.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use parking_lot::Mutex;

use crate::media::ResolvedSource;
use crate::playback::bus::Emitter;
use crate::playback::gst::{InternalAspectRatioMode, InternalVideoMetadata};
use crate::playback::overlay::OverlaySession;
use crate::playback::replay::PlayReplayContext;
use crate::playback::shell::{
    install_asset_shell, install_uri_shell, teardown_shell, wire_overlay_sync, PipelineShell,
};
#[cfg(target_os = "android")]
use crate::playback::sink::OverlaySizeSync;
use crate::playback::surface::VideoSurface;
use crate::playback::tracks::TrackCache;

/// URI ↔ 资产 shell 切换时的管线元数据（不含 replay 原子量与 surface）/ Pipeline-only metadata for URI ↔ asset shell swaps.
#[derive(Clone)]
pub struct PipelineSwapConfig {
    /// 事件发射器 / Event emitter.
    pub emitter: Arc<Mutex<Option<Emitter>>>,
    /// 循环播放标志 / Looping flag.
    pub looping: Arc<AtomicBool>,
    /// 视频元数据缓存 / Video metadata cache.
    pub metadata: Arc<Mutex<InternalVideoMetadata>>,
    /// 多轨缓存 / Multi-track cache.
    pub track_cache: Arc<Mutex<TrackCache>>,
    /// 顺时针旋转角度快照：0、90、180 或 270 / Clockwise rotation degrees snapshot.
    pub rotate_degrees: i32,
    /// 宽高比模式快照 / Aspect ratio mode snapshot.
    pub aspect: InternalAspectRatioMode,
    /// 跨 shell 重建复用的帧源，保证 URI ↔ 资产切换后 Flutter 纹理仍收帧 / Frame source reused across shell rebuilds.
    pub frame_sink: Arc<crate::playback::frame::FrameSink>,
    /// 解码尺寸变更触发的 ImageReader 调整与 overlay 同步（Android 纹理路径）/ Caps-driven overlay sync (Android texture path).
    #[cfg(target_os = "android")]
    pub overlay_size_sync: Option<OverlaySizeSync>,
}

impl PipelineSwapConfig {
    /// 克隆供 Gst 异步闭包使用 / Clones for async Gst closures.
    ///
    /// # 参数 / Parameters
    /// - 无 / None
    ///
    /// # 返回值 / Returns
    /// - 共享 Arc 的新 [`PipelineSwapConfig`] / new config sharing Arc pointers
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 任意线程 / any thread
    ///
    /// # 平台 / Platform
    /// - Android 克隆 `overlay_size_sync` / clones `overlay_size_sync` on Android
    pub fn clone_for_async(&self) -> Self {
        Self {
            emitter: self.emitter.clone(),
            looping: self.looping.clone(),
            metadata: self.metadata.clone(),
            track_cache: self.track_cache.clone(),
            rotate_degrees: self.rotate_degrees,
            aspect: self.aspect,
            frame_sink: self.frame_sink.clone(),
            #[cfg(target_os = "android")]
            overlay_size_sync: self.overlay_size_sync.clone(),
        }
    }
}

/// 为 `resolved` 重建或重配置 pipeline shell 并应用 overlay/orientation / Rebuilds or reconfigures the pipeline shell for `resolved`.
///
/// # 参数 / Parameters
/// - `shell` — 可变 pipeline shell / mutable pipeline shell
/// - `resolved` — 已解析的 URI 或 AppSrc 资产 / resolved URI or AppSrc asset
/// - `swap` — 跨重建元数据 / metadata across rebuilds
/// - `replay` — 重放上下文 / replay context
/// - `surface` — VideoSurface / video surface
///
/// # 返回值 / Returns
/// - 成功：`Ok(())` / `Ok(())`
///
/// # 错误 / Errors
/// - shell 安装、URI 设置或 preroll 失败 / shell install, URI set, or preroll failure
///
/// # 线程 / Threading
/// - 必须在 Gst 线程上调用 / Must run on Gst thread
///
/// # 平台 / Platform
/// - macOS/iOS 额外同步 overlay sink slot / macOS/iOS additionally sync overlay sink slot
pub fn switch_shell(
    shell: &mut PipelineShell,
    resolved: ResolvedSource,
    swap: &PipelineSwapConfig,
    replay: &PlayReplayContext,
    surface: &VideoSurface,
) -> Result<()> {
    match resolved {
        ResolvedSource::Uri(uri) => switch_uri_shell(shell, &uri, swap, replay, surface),
        ResolvedSource::AppSrc(asset_key) => {
            switch_asset_shell(shell, &asset_key, swap, replay, surface)
        }
    }
}

fn switch_uri_shell(
    shell: &mut PipelineShell,
    uri: &str,
    swap: &PipelineSwapConfig,
    replay: &PlayReplayContext,
    surface: &VideoSurface,
) -> Result<()> {
    if !shell.is_uri() {
        teardown_shell(shell);
        surface.mark_shell_rebuilt();
        *shell = install_uri_shell(
            &swap.emitter,
            &swap.looping,
            replay,
            Some(swap.metadata.clone()),
            Some(swap.track_cache.clone()),
            surface,
            &swap.frame_sink,
            #[cfg(target_os = "android")]
            swap.overlay_size_sync.clone(),
        )?;
        #[cfg(target_os = "ios")]
        {
            let overlay_sink = surface.overlay_sink_slot().cloned();
            wire_overlay_sync(shell, surface.stored_handle(), overlay_sink);
            if let Some(slot) = surface.overlay_sink_slot() {
                shell.sync_overlay_sink_slot(slot);
            }
        }
        #[cfg(not(target_os = "ios"))]
        wire_overlay_sync(shell, surface.stored_handle());
    }
    surface.rebind_cached_overlay(shell)?;
    shell.apply_aspect_ratio(swap.aspect);
    shell.apply_rotation(swap.rotate_degrees)?;
    pipeline_set_uri(shell, uri, replay, surface)
}

/// 切换到 AppSrc 资产 shell（总是完整重建）/ Switches to AppSrc asset shell (always full rebuild).
///
/// # 参数 / Parameters
/// - `shell`、`asset_key`、`swap`、`replay`、`surface` — 同 [`switch_shell`] / same as [`switch_shell`]
///
/// # 返回值 / Returns
/// - 成功：`Ok(())` / `Ok(())`
///
/// # 错误 / Errors
/// - [`install_asset_shell`] 或 preroll 失败 / install or preroll failure
///
/// # 线程 / Threading
/// - Gst 线程 / Gst thread
///
/// # 平台 / Platform
/// - Android 可能在 overlay 未绑定时延迟 Paused preroll / Android may defer Paused preroll until overlay bound
pub(crate) fn switch_asset_shell(
    shell: &mut PipelineShell,
    asset_key: &str,
    swap: &PipelineSwapConfig,
    replay: &PlayReplayContext,
    surface: &VideoSurface,
) -> Result<()> {
    teardown_shell(shell);
    surface.mark_shell_rebuilt();
    *shell = install_asset_shell(
        asset_key,
        &swap.emitter,
        &swap.looping,
        replay,
        Some(swap.metadata.clone()),
        surface,
        &swap.frame_sink,
        #[cfg(target_os = "android")]
        swap.overlay_size_sync.clone(),
    )?;
    #[cfg(target_os = "ios")]
    {
        let overlay_sink = surface.overlay_sink_slot().cloned();
        wire_overlay_sync(shell, surface.stored_handle(), overlay_sink);
        if let Some(slot) = surface.overlay_sink_slot() {
            shell.sync_overlay_sink_slot(slot);
        }
    }
    #[cfg(not(target_os = "ios"))]
    wire_overlay_sync(shell, surface.stored_handle());
    surface.rebind_cached_overlay(shell)?;
    shell.apply_aspect_ratio(swap.aspect);
    shell.apply_rotation(swap.rotate_degrees)?;
    replay.at_eos.store(false, Ordering::SeqCst);
    preroll_asset_shell(
        shell,
        surface,
        "gst: deferring asset Paused preroll until Android overlay is bound",
    )
}

fn preroll_asset_shell(
    shell: &PipelineShell,
    surface: &VideoSurface,
    defer_log: &str,
) -> Result<()> {
    surface
        .overlay_session()
        .apply_load_preroll(shell, surface, defer_log)
}

fn pipeline_set_uri(
    shell: &PipelineShell,
    uri: &str,
    replay: &PlayReplayContext,
    surface: &VideoSurface,
) -> Result<()> {
    replay.at_eos.store(false, Ordering::SeqCst);
    shell.set_uri(uri)?;
    surface.overlay_session().apply_load_preroll(
        shell,
        surface,
        "gst: deferring URI Paused preroll until Android overlay is bound",
    )
}
