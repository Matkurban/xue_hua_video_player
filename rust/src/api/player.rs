//! FRB 播放器控制入口 / FRB player control entry points.
//!
//! Dart 侧所有播放命令经本模块路由到 `PlaybackEngine` 实例。
//! 全局 `PLAYERS` 表以 `player_id` 寻址；overlay 句柄可在 `create_player` 之前
//! 暂存于 `PENDING_OVERLAYS`。
//!
//! All Dart playback commands route through this module to `PlaybackEngine` instances.
//! Global `PLAYERS` map is keyed by `player_id`; overlay handles may be staged in
//! `PENDING_OVERLAYS` before `create_player`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use parking_lot::Mutex;

pub use crate::api::types::{
    AspectRatioMode, MediaSourceDto, MediaTrack, PipelineCapabilitiesDto, PlayerEvent,
    PlayerEventKind, PlayerState, TrackType, VideoMetadata, VideoOrientationConfig,
};
use crate::frb_generated::StreamSink;
use crate::playback::PlaybackEngine;

/// 活跃播放器实例表：`player_id` → [`PlaybackEngine`] / Active player instances keyed by `player_id`.
static PLAYERS: Lazy<Mutex<HashMap<i64, Arc<PlaybackEngine>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// 在 `create_player` 之前到达的 overlay 原生句柄 / Native overlay handles received before `create_player`.
static PENDING_OVERLAYS: Lazy<Mutex<HashMap<i64, i64>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// 单调递增的下一个 `player_id` / Monotonically increasing next `player_id`.
static NEXT_ID: AtomicI64 = AtomicI64::new(1);

/// 创建播放器时返回的标识符 / Identifiers returned when a player is created.
///
/// `player_id` 用于所有后续 FRB 控制调用；Platform View 应通过 `creationParams` 绑定相同 id。
/// `player_id` addresses all subsequent control calls; bind a Platform View with the same id via `creationParams`.
pub struct PlayerHandle {
    /// 播放器唯一 ID / Unique player identifier.
    pub player_id: i64,
}

/// 按 ID 查找播放器 / Look up a player by id.
///
/// # 参数 / Parameters
/// - `id` — 由 [`create_player`] 分配的 `player_id` / `player_id` from [`create_player`]
///
/// # 返回值 / Returns
/// - 成功：`Arc<PlaybackEngine>` 共享引用 / shared reference to the engine
///
/// # 错误 / Errors
/// - 播放器不存在或已 dispose / player not found or already disposed
fn get_player(id: i64) -> Result<Arc<PlaybackEngine>> {
    PLAYERS
        .lock()
        .get(&id)
        .cloned()
        .ok_or_else(|| anyhow!("player {id} not found (already disposed?)"))
}

/// 创建 GStreamer 播放器实例 / Creates a new GStreamer-backed player.
///
/// # 参数 / Parameters
/// - 无 / None
///
/// # 返回值 / Returns
/// - 成功：[`PlayerHandle`]，含新分配的 `player_id` / handle with new `player_id`
///
/// # 错误 / Errors
/// - `PlaybackEngine::new` 或 GStreamer 初始化失败 / engine or GStreamer init failure
///
/// # 平台 / Platform
/// - Android：初始化诊断日志并注册 `player_id` 到 texture 桥 / initializes diagnostics and registers texture bridge
/// - 若 `PENDING_OVERLAYS` 中有待处理句柄，创建后立即应用 / applies staged overlay handle if present
pub fn create_player() -> Result<PlayerHandle> {
    #[cfg(target_os = "android")]
    crate::diag::ensure_android_diagnostics_initialized();

    let player = PlaybackEngine::new()?;
    let player_id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    crate::playback::frame::register_frame_sink(player_id, player.frame_sink());
    #[cfg(target_os = "android")]
    player.set_player_id(player_id);
    PLAYERS.lock().insert(player_id, Arc::new(player));
    if let Some(handle) = PENDING_OVERLAYS.lock().remove(&player_id) {
        #[cfg(target_os = "macos")]
        let _ = handle;
        #[cfg(target_os = "android")]
        get_player(player_id)?.notify_android_surface(handle, 0, 0)?;
        #[cfg(target_os = "ios")]
        {
            get_player(player_id)?.notify_ios_overlay(handle, 0, 0)?;
            let _ = apply_ios_overlay_gstreamer(player_id, 0, 0);
        }
        #[cfg(all(
            not(target_os = "android"),
            not(target_os = "ios"),
            not(target_os = "macos")
        ))]
        get_player(player_id)?.set_video_overlay_window(handle)?;
    }
    Ok(PlayerHandle { player_id })
}

/// iOS：缓存 EaglUIView 句柄，pipeline READY 后在 `xhvp-gst` 绑定 / iOS: caches EaglUIView handle and binds on xhvp-gst after pipeline READY.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `handle` — 原生视图句柄 / native view handle
/// - `width` — 布局宽（像素）/ layout width in pixels
/// - `height` — 布局高（像素）/ layout height in pixels
///
/// # 返回值 / Returns
/// - 成功：`Ok(())` / `Ok(())`
#[cfg(target_os = "ios")]
pub fn notify_ios_overlay(player_id: i64, handle: i64, width: i32, height: i32) -> Result<()> {
    match get_player(player_id) {
        Ok(player) => {
            PENDING_OVERLAYS.lock().remove(&player_id);
            player.notify_ios_overlay(handle, width, height)
        }
        Err(_) if handle != 0 => {
            PENDING_OVERLAYS.lock().insert(player_id, handle);
            Ok(())
        }
        Err(_) => {
            PENDING_OVERLAYS.lock().remove(&player_id);
            Ok(())
        }
    }
}

/// iOS 占位 / iOS stub.
#[cfg(not(target_os = "ios"))]
pub fn notify_ios_overlay(_player_id: i64, _handle: i64, _width: i32, _height: i32) -> Result<()> {
    Ok(())
}

/// iOS：应用缓存的 host view 并 attach `avsamplebufferlayersink` layer（主线程）/ iOS: applies cached host view + attaches layer (main thread).
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `width` — 布局宽 / layout width
/// - `height` — 布局高 / layout height
#[cfg(target_os = "ios")]
pub fn apply_ios_overlay_gstreamer(player_id: i64, width: i32, height: i32) -> Result<()> {
    get_player(player_id)?.apply_ios_overlay_gstreamer(width, height)
}

/// iOS 占位 / iOS stub.
#[cfg(not(target_os = "ios"))]
pub fn apply_ios_overlay_gstreamer(_player_id: i64, _width: i32, _height: i32) -> Result<()> {
    Ok(())
}

/// 将原生窗口/表面句柄绑定到 VideoOverlay sink / Binds a native window/surface handle to the player's VideoOverlay sink.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `window_handle` — 原生窗口句柄（Win/Linux 等）/ native window handle
///
/// # 返回值 / Returns
/// - 成功：`Ok(())`；播放器未创建时非零句柄暂存 / staged if player not yet created
///
/// # 错误 / Errors
/// - 播放器不存在且句柄为 0 / player not found with zero handle
pub fn set_video_overlay_window(player_id: i64, window_handle: i64) -> Result<()> {
    match get_player(player_id) {
        Ok(player) => {
            PENDING_OVERLAYS.lock().remove(&player_id);
            player.set_video_overlay_window(window_handle)
        }
        Err(_) if window_handle != 0 => {
            PENDING_OVERLAYS.lock().insert(player_id, window_handle);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Android：在 JNI 线程缓存 `ANativeWindow`，在 `xhvp-gst` 应用 VideoOverlay，不阻塞主线程 / Android: caches ANativeWindow on JNI thread, applies overlay on xhvp-gst.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `handle` — `ANativeWindow` 指针 / ANativeWindow pointer
/// - `width` — 表面宽 / surface width
/// - `height` — 表面高 / surface height
#[cfg(target_os = "android")]
pub fn notify_android_surface(player_id: i64, handle: i64, width: i32, height: i32) -> Result<()> {
    match get_player(player_id) {
        Ok(player) => {
            PENDING_OVERLAYS.lock().remove(&player_id);
            player.notify_android_surface(handle, width, height)
        }
        Err(_) if handle != 0 => {
            PENDING_OVERLAYS.lock().insert(player_id, handle);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Android 占位 / Android stub.
#[cfg(not(target_os = "android"))]
pub fn notify_android_surface(
    _player_id: i64,
    _handle: i64,
    _width: i32,
    _height: i32,
) -> Result<()> {
    Ok(())
}

/// 订阅播放器事件流 / Subscribes to the player's event stream.
///
/// 推送状态、位置、时长、尺寸、缓冲、EOS、错误等事件。应在 `create_player` 后立即调用一次。
/// Pushes state, position, duration, size, buffering, EOS, errors. Call once right after `create_player`.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `sink` — FRB 广播流 sink / FRB broadcast stream sink
///
/// # 返回值 / Returns
/// - 成功：`Ok(())` / `Ok(())`
///
/// # 错误 / Errors
/// - 播放器不存在 / player not found
pub fn player_event_stream(player_id: i64, sink: StreamSink<PlayerEvent>) -> Result<()> {
    let player = get_player(player_id)?;
    player.set_emitter(Arc::new(move |event| {
        let _ = sink.add(event);
    }));
    Ok(())
}

/// 从统一源描述符加载媒体 / Loads media from a unified source descriptor (URI or Flutter asset).
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `source` — URI 或 Flutter asset / URI or Flutter asset
/// - `auto_play` — 加载完成后是否自动播放 / whether to start playback after load
///
/// # 错误 / Errors
/// - 解析失败、pipeline 切换失败等 / resolution or pipeline switch failure
pub fn player_load_source(player_id: i64, source: MediaSourceDto, auto_play: bool) -> Result<()> {
    get_player(player_id)?.load(source.into(), auto_play)
}

/// 加载媒体 URI 并 preroll（不自动播放）/ Loads a media URI and prerolls without auto-play.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `uri` — `file://...`、`http(s)://...`、`rtsp://...` 等 / media URI
pub fn player_set_source(player_id: i64, uri: String) -> Result<()> {
    get_player(player_id)?.load(MediaSourceDto::Uri(uri).into(), false)
}

/// 通过 AppSrc 加载 Flutter asset（Dart 侧无需临时文件）/ Loads a Flutter asset key via AppSrc (no Dart-side temp file copy).
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `asset_key` — 如 `assets/sample.mp4` / asset key
pub fn player_set_asset_source(player_id: i64, asset_key: String) -> Result<()> {
    get_player(player_id)?.load(MediaSourceDto::FlutterAsset(asset_key).into(), false)
}

/// 开始或恢复播放 / Starts or resumes playback.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
pub fn player_play(player_id: i64) -> Result<()> {
    get_player(player_id)?.play()
}

/// 暂停播放 / Pauses playback.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
pub fn player_pause(player_id: i64) -> Result<()> {
    get_player(player_id)?.pause()
}

/// 停止播放（pipeline → NULL）/ Stops playback (pipeline to NULL).
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
pub fn player_stop(player_id: i64) -> Result<()> {
    get_player(player_id)?.stop()
}

/// 跳转到指定位置 / Seeks to a position.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `position_ms` — 目标位置（毫秒）/ target position in milliseconds
pub fn player_seek(player_id: i64, position_ms: i64) -> Result<()> {
    get_player(player_id)?.seek(position_ms)
}

/// 设置音量 / Sets playback volume.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `volume` — 0.0–1.0（具体 clamp 由 engine 负责）/ 0.0–1.0, clamped by engine
pub fn player_set_volume(player_id: i64, volume: f64) -> Result<()> {
    get_player(player_id)?.set_volume(volume);
    Ok(())
}

/// 设置静音 / Sets mute state.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `mute` — `true` 静音 / `true` to mute
pub fn player_set_mute(player_id: i64, mute: bool) -> Result<()> {
    get_player(player_id)?.set_mute(mute);
    Ok(())
}

/// 设置播放速率 / Sets playback speed.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `speed` — 倍速，如 1.0、1.5 / speed multiplier
pub fn player_set_speed(player_id: i64, speed: f64) -> Result<()> {
    get_player(player_id)?.set_speed(speed)
}

/// 设置循环播放 / Sets looping.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
/// - `looping` — `true` 在 EOS 时循环 / `true` to loop at EOS
pub fn player_set_looping(player_id: i64, looping: bool) -> Result<()> {
    get_player(player_id)?.set_looping(looping);
    Ok(())
}

/// 查询当前播放位置 / Returns current playback position.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player id
///
/// # 返回值 / Returns
/// - 位置（毫秒）/ position in milliseconds
pub fn player_position(player_id: i64) -> Result<i64> {
    Ok(get_player(player_id)?.position_ms())
}

/// 查询媒体总时长 / Returns media duration.
///
/// # 返回值 / Returns
/// - 时长（毫秒），未知时可能为 0 / duration in ms, may be 0 if unknown
pub fn player_duration(player_id: i64) -> Result<i64> {
    Ok(get_player(player_id)?.duration_ms())
}

/// 查询是否可 seek / Returns whether seeking is supported.
pub fn player_is_seekable(player_id: i64) -> Result<bool> {
    Ok(get_player(player_id)?.is_seekable())
}

/// 查询当前 pipeline 能力 / Returns active pipeline capabilities.
pub fn player_get_pipeline_capabilities(player_id: i64) -> Result<PipelineCapabilitiesDto> {
    Ok(get_player(player_id)?.pipeline_capabilities().into())
}

/// 同步 VideoOverlay 矩形尺寸 / Syncs VideoOverlay rectangle dimensions.
///
/// # 参数 / Parameters
/// - `width` — overlay 宽（像素）/ width in pixels
/// - `height` — overlay 高（像素）/ height in pixels
pub fn sync_video_overlay_rectangle(player_id: i64, width: i32, height: i32) -> Result<()> {
    get_player(player_id)?.sync_video_overlay_rectangle(width, height)
}

/// 获取可用轨道列表 / Returns available media tracks.
pub fn player_get_tracks(player_id: i64) -> Result<Vec<MediaTrack>> {
    Ok(get_player(player_id)?.tracks())
}

/// 选择或取消选择轨道 / Selects or deselects a track.
///
/// # 参数 / Parameters
/// - `track_id` — GStreamer 流 ID / stream id
/// - `track_type` — 轨道类型 / track type
/// - `enable` — `true` 启用该轨 / `true` to enable
pub fn player_select_track(
    player_id: i64,
    track_id: u32,
    track_type: TrackType,
    enable: bool,
) -> Result<()> {
    get_player(player_id)?.select_track(track_id, track_type, enable)
}

/// 获取当前视频元数据 / Returns current video metadata.
pub fn player_get_video_metadata(player_id: i64) -> Result<VideoMetadata> {
    let meta = get_player(player_id)?.video_metadata();
    Ok(meta)
}

/// 设置视频方向（翻转/旋转）/ Sets video orientation (flip/rotate).
pub fn player_set_video_orientation(player_id: i64, config: VideoOrientationConfig) -> Result<()> {
    get_player(player_id)?.set_video_orientation(config.into())
}

/// 设置宽高比缩放模式 / Sets aspect ratio scaling mode.
pub fn player_set_aspect_ratio_mode(player_id: i64, mode: AspectRatioMode) -> Result<()> {
    get_player(player_id)?.set_aspect_ratio_mode(mode.into())
}

/// 销毁播放器并停止 pipeline / Tears down the player and stops the pipeline.
///
/// # 参数 / Parameters
/// - `player_id` — 要销毁的 ID / id to dispose
///
/// # 返回值 / Returns
/// - 始终 `Ok(())`（幂等，重复 dispose 不报错）/ always `Ok(())` (idempotent)
pub fn dispose_player(player_id: i64) -> Result<()> {
    PENDING_OVERLAYS.lock().remove(&player_id);
    crate::playback::frame::unregister_frame_sink(player_id);
    PLAYERS.lock().remove(&player_id);
    Ok(())
}
