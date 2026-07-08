//! Pipeline shell：sink、总线、overlay 同步 / Pipeline shell: sinks, bus handlers, and overlay sync wiring.
//!
//! [`PipelineShell`] 封装 playbin 或 AppSrc 管线的 GStreamer 元素、视频 sink、总线监听与
//! seek/音量/多轨等操作；[`install_uri_shell`] / [`install_asset_shell`] 在引擎初始化或
//! [`crate::playback::switch::switch_shell`] 时安装 shell。
//!
//! [`PipelineShell`] wraps GStreamer elements, video sink, bus watches, and seek/volume/track
//! operations for playbin or AppSrc pipelines; installed by engine init or
//! [`crate::playback::switch::switch_shell`].

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer::StateChangeSuccess;
use parking_lot::Mutex;

use crate::media::AppSrcFeedState;
use crate::playback::asset_pipeline::build_asset_pipeline;
use crate::playback::bus::{attach_gst_bus_handlers, Emitter};
use crate::playback::capabilities::PipelineCapabilities;
use crate::playback::gst::apply_rotation_to_playbin;
use crate::playback::gst::attach_overlay_bus_sync_handler;
use crate::playback::gst::{
    apply_rotation_to_element, expose_overlay, flush_videoflip_element,
    set_overlay_render_rectangle, set_overlay_window_handle, InternalAspectRatioMode,
    InternalVideoMetadata,
};
use crate::playback::overlay::PipelineSnapshot;
use crate::playback::replay::PlayReplayContext;
#[cfg(target_os = "android")]
use crate::playback::sink::OverlaySizeSync;
use crate::playback::surface::VideoSurface;
use crate::playback::tracks::{
    disable_subtitles_on_pipeline, select_track_on_pipeline, TrackCache,
};
use crate::playback::uri_pipeline::build_uri_playbin;
use crate::player_events::TrackType;

const DEFAULT_STATE_TIMEOUT: gst::ClockTime = gst::ClockTime::from_seconds(10);

/// 媒体源类型：URI playbin 或 Flutter 资产 AppSrc / Media source kind: URI playbin or Flutter asset AppSrc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    /// 网络/文件 URI（playbin3）/ Network/file URI (playbin3).
    Uri,
    /// Flutter bundle 资产（AppSrc）/ Flutter bundle asset (AppSrc).
    Asset,
}

/// 共享 pipeline shell：sink、总线处理与 overlay 同步 / Shared pipeline shell: sinks, bus handlers, and overlay sync.
pub struct PipelineShell {
    pipeline: gst::Pipeline,
    video_sink: gst::Element,
    kind: SourceKind,
    is_playbin: bool,
    asset_key: Option<String>,
    appsrc_feed: Option<Arc<AppSrcFeedState>>,
    bus_watch: Option<gst::bus::BusWatchGuard>,
    position_source: Option<gst::glib::SourceId>,
    /// 缓存的 `videoflip`（playbin video-filter 或 AppSrc 支路）/ Cached `videoflip` for playbin or AppSrc.
    orientation_filter: Option<gst::Element>,
}

impl PipelineShell {
    pub fn source_kind(&self) -> SourceKind {
        self.kind
    }

    pub fn is_uri(&self) -> bool {
        self.kind == SourceKind::Uri
    }

    pub fn asset_key(&self) -> Option<&str> {
        self.asset_key.as_deref()
    }

    pub fn capabilities(&self) -> PipelineCapabilities {
        PipelineCapabilities::from_source_kind(self.kind)
    }

    /// shell 是否有待加载媒体（避免 overlay 过早 preroll）/ Whether shell has media ready for preroll.
    ///
    /// 空 playbin（URI 未设置）返回 `false`，防止 `load()` 前绑定 overlay 导致 panic。
    ///
    /// An empty playbin (`SourceKind::Uri` with `uri` unset) returns `false` so early overlay bind does not panic.
    ///
    /// # 参数 / Parameters
    /// - 无 / None
    ///
    /// # 返回值 / Returns
    /// - `true` 当 URI 或 asset key 非空 / `true` when URI or asset key is non-empty
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - Gst 线程 / Gst thread
    ///
    /// # 平台 / Platform
    /// - 与平台无关 / Platform-independent
    pub fn has_pending_media(&self) -> bool {
        match self.kind {
            SourceKind::Uri => self
                .pipeline
                .property::<Option<String>>("uri")
                .is_some_and(|uri| !uri.is_empty()),
            SourceKind::Asset => self.asset_key.as_ref().is_some_and(|key| !key.is_empty()),
        }
    }

    pub fn snapshot(&self) -> PipelineSnapshot {
        let (_, current, pending) = self.pipeline.state(gst::ClockTime::ZERO);
        PipelineSnapshot {
            current,
            pending,
            has_pending_media: self.has_pending_media(),
        }
    }

    pub fn set_state_sync(&self, target: gst::State) -> Result<()> {
        set_element_state_sync(&self.pipeline, target)
    }

    pub fn set_uri(&self, uri: &str) -> Result<()> {
        self.set_state_sync(gst::State::Ready)?;
        self.pipeline.set_property("uri", uri);
        Ok(())
    }

    /// 以指定速率 seek 到起点（EOS 重放/循环保持用户选速）/ Seeks to start carrying `rate` for EOS replay/loop.
    ///
    /// 普通 `seek_simple` 会将速率重置为 1.0；带 rate 的 seek 让 scaletempo 获得速率段并保持音高。
    ///
    /// # 参数 / Parameters
    /// - `rate` — 播放速率 / playback rate
    ///
    /// # 返回值 / Returns
    /// - 成功：`Ok(())` / `Ok(())`
    ///
    /// # 错误 / Errors
    /// - seek 失败 / seek failure
    ///
    /// # 线程 / Threading
    /// - Gst 线程 / Gst thread
    ///
    /// # 平台 / Platform
    /// - 与平台无关 / Platform-independent
    pub fn seek_to_start_with_rate(&self, rate: f64) -> Result<()> {
        self.pipeline
            .seek(
                rate,
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::SeekType::Set,
                gst::ClockTime::ZERO,
                gst::SeekType::None,
                gst::ClockTime::ZERO,
            )
            .map_err(|e| anyhow!("seek to start (rate {rate}) failed: {e}"))
    }

    pub fn seek_accurate(&self, position_ms: i64, rate: f64) -> Result<()> {
        let pos = gst::ClockTime::from_mseconds(position_ms.max(0) as u64);
        self.pipeline
            .seek(
                rate,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::SeekType::Set,
                pos,
                gst::SeekType::None,
                gst::ClockTime::ZERO,
            )
            .map_err(|e| anyhow!("seek failed: {e}"))
    }

    /// 应用播放速率（位置保持的 flushing rate seek，经 scaletempo 保 pitch）/ Applies playback rate via position-preserving rate seek.
    ///
    /// # 参数 / Parameters
    /// - `rate` — 目标速率 / target rate
    ///
    /// # 返回值 / Returns
    /// - 成功：`Ok(())` / `Ok(())`
    ///
    /// # 错误 / Errors
    /// - seek 失败 / seek failure
    ///
    /// # 线程 / Threading
    /// - Gst 线程 / Gst thread
    ///
    /// # 平台 / Platform
    /// - 依赖音频 bin 中的 scaletempo / depends on scaletempo in audio bin
    pub fn apply_playback_rate(&self, rate: f64) -> Result<()> {
        // Position-preserving flushing rate seek (not INSTANT_RATE_CHANGE): this
        // sends a rate-bearing segment so scaletempo time-stretches and keeps the
        // original pitch. INSTANT_RATE_CHANGE bypasses scaletempo and shifts pitch.
        self.seek_accurate(self.query_position_ms(), rate)
    }

    pub fn set_volume(&self, volume: f64) {
        self.pipeline.set_property("volume", volume);
    }

    pub fn set_mute(&self, mute: bool) {
        self.pipeline.set_property("mute", mute);
    }

    pub fn query_position_ms(&self) -> i64 {
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map(|p| p.mseconds() as i64)
            .unwrap_or(0)
    }

    pub fn query_duration_ms(&self) -> i64 {
        self.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|d| d.mseconds() as i64)
            .unwrap_or(0)
    }

    pub fn apply_aspect_ratio(&self, mode: InternalAspectRatioMode) {
        mode.apply_to_sink(&self.video_sink);
    }

    pub fn apply_rotation(&mut self, rotate_degrees: i32) -> Result<()> {
        let was_playing = self.pipeline.current_state() == gst::State::Playing;
        if was_playing && self.is_playbin {
            self.set_state_sync(gst::State::Paused)?;
        }

        if self.is_playbin {
            apply_rotation_to_playbin(
                self.pipeline.upcast_ref::<gst::Element>(),
                rotate_degrees,
                &mut self.orientation_filter,
            )?;
        } else if let Some(ref flip) = self.orientation_filter {
            apply_rotation_to_element(flip, rotate_degrees)?;
            if rotate_degrees == 0 {
                flush_videoflip_element(flip)?;
            }
        }

        if was_playing && self.is_playbin {
            self.set_state_sync(gst::State::Playing)?;
        }
        Ok(())
    }

    pub fn apply_overlay_window_handle(&self, handle: usize) -> Result<()> {
        set_overlay_window_handle(&self.video_sink, handle)
    }

    pub fn apply_overlay_render_rectangle(&self, width: i32, height: i32) {
        if width > 0 && height > 0 {
            set_overlay_render_rectangle(&self.video_sink, width, height);
        }
    }

    pub fn expose_video_overlay(&self) {
        expose_overlay(&self.video_sink);
    }

    pub fn sync_overlay_sink_slot(&self, slot: &Arc<Mutex<gst::Element>>) {
        *slot.lock() = self.video_sink().clone();
    }

    pub fn disable_subtitles(&self, cache: &TrackCache) {
        disable_subtitles_on_pipeline(&self.pipeline, cache);
    }

    pub fn select_track(&self, cache: &TrackCache, track_type: TrackType, track_id: u32) {
        select_track_on_pipeline(&self.pipeline, cache, track_type, track_id);
    }

    #[cfg(target_os = "ios")]
    pub fn preroll_for_ios_layer(&self) -> Result<()> {
        if !self.has_pending_media() {
            return Ok(());
        }
        crate::platform::ios::layer::preroll_pipeline_for_ios_layer(&self.pipeline)
    }

    pub(crate) fn clone_video_sink(&self) -> gst::Element {
        self.video_sink.clone()
    }

    pub(crate) fn clone_pipeline(&self) -> gst::Pipeline {
        self.pipeline.clone()
    }

    pub(crate) fn video_sink(&self) -> &gst::Element {
        &self.video_sink
    }

    pub(crate) fn pipeline_bus(&self) -> Option<gst::Bus> {
        self.pipeline.bus()
    }

    pub(crate) fn set_state_null(&self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

#[cfg(test)]
pub(crate) fn new_test_shell(
    pipeline: gst::Pipeline,
    video_sink: gst::Element,
    kind: SourceKind,
    asset_key: Option<String>,
) -> PipelineShell {
    PipelineShell {
        pipeline,
        video_sink,
        kind,
        is_playbin: kind == SourceKind::Uri,
        asset_key,
        appsrc_feed: None,
        bus_watch: None,
        position_source: None,
        orientation_filter: None,
    }
}

/// 设置元素状态并等待转换完成 / Sets pipeline/element state and waits until the transition completes.
pub(crate) fn set_element_state_sync(
    element: &impl IsA<gst::Element>,
    target: gst::State,
) -> Result<()> {
    set_element_state_sync_timeout(element, target, DEFAULT_STATE_TIMEOUT)
}

pub(crate) fn set_element_state_sync_timeout(
    element: &impl IsA<gst::Element>,
    target: gst::State,
    timeout: gst::ClockTime,
) -> Result<()> {
    let element = element.upcast_ref::<gst::Element>();
    let change = element.set_state(target).map_err(|e| {
        let msg = format!("set_state({target:?}) failed: {e}");
        log::error!("{msg}");
        #[cfg(target_os = "android")]
        crate::diag::logcat_error(&format!("gst: {msg}"));
        anyhow!("{msg}")
    })?;
    if matches!(change, StateChangeSuccess::Success) {
        return Ok(());
    }
    let (ret, current, _pending) = element.state(Some(timeout));
    ret.map_err(|e| {
        let msg = format!("get_state after set_state({target:?}) failed: {e}");
        log::error!("{msg}");
        #[cfg(target_os = "android")]
        crate::diag::logcat_error(&format!("gst: {msg}"));
        anyhow!("{msg}")
    })?;
    if current != target {
        let msg = format!(
            "element failed to change state to {target:?} (current {current:?}) within {timeout:?}"
        );
        log::error!("{msg}");
        #[cfg(target_os = "android")]
        crate::diag::logcat_error(&format!("gst: {msg}"));
        return Err(anyhow!("{msg}"));
    }
    Ok(())
}

/// 安装 URI playbin shell（引擎初始化或 URI 切换）/ Installs URI playbin shell (engine init or URI switch).
///
/// # 参数 / Parameters
/// - `emitter`、`looping`、`replay` — 总线与重放上下文 / bus and replay context
/// - `metadata_cache`、`track_cache` — 可选缓存 / optional caches
/// - `surface` — VideoSurface / video surface
/// - `frame_sink` — 外部纹理帧源 / external texture frame source
/// - `overlay_size_sync`（Android）— 尺寸同步 / size sync on Android
///
/// # 返回值 / Returns
/// - 成功：[`PipelineShell`] / configured shell
///
/// # 错误 / Errors
/// - [`build_uri_playbin`] 或总线挂载失败 / pipeline or bus attach failure
///
/// # 线程 / Threading
/// - Gst 线程 / Gst thread
///
/// # 平台 / Platform
/// - iOS 传入 layer bus slot / passes iOS layer bus slot
pub fn install_uri_shell(
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    replay: &PlayReplayContext,
    metadata_cache: Option<Arc<Mutex<InternalVideoMetadata>>>,
    track_cache: Option<Arc<Mutex<TrackCache>>>,
    surface: &VideoSurface,
    frame_sink: &Arc<crate::playback::frame::FrameSink>,
    #[cfg(target_os = "android")] overlay_size_sync: Option<OverlaySizeSync>,
) -> Result<PipelineShell> {
    let (pipeline, video_sink) = build_uri_playbin(
        emitter,
        metadata_cache,
        frame_sink,
        #[cfg(target_os = "android")]
        overlay_size_sync,
    )?;
    let (bus_watch, position_source) = attach_gst_bus_handlers(
        &pipeline,
        emitter,
        looping,
        &replay.desired_playing,
        &replay.at_eos,
        &replay.running,
        &replay.rate,
        true,
        track_cache,
        #[cfg(target_os = "ios")]
        Some(surface.ios_layer_bus_slot()),
    )?;
    Ok(PipelineShell {
        pipeline,
        video_sink,
        kind: SourceKind::Uri,
        is_playbin: true,
        asset_key: None,
        appsrc_feed: None,
        bus_watch: Some(bus_watch),
        position_source: Some(position_source),
        orientation_filter: None,
    })
}

/// 安装 AppSrc 资产 shell / Installs AppSrc asset shell.
///
/// # 参数 / Parameters
/// - `asset_key` — Flutter asset 键 / Flutter asset key
/// - 其余同 [`install_uri_shell`] / remaining args same as [`install_uri_shell`]
///
/// # 返回值 / Returns
/// - 成功：[`PipelineShell`] / configured shell
///
/// # 错误 / Errors
/// - [`build_asset_pipeline`] 或总线挂载失败 / pipeline or bus failure
///
/// # 线程 / Threading
/// - Gst 线程 / Gst thread
///
/// # 平台 / Platform
/// - 无多轨缓存（AppSrc 能力受限）/ no track cache (limited AppSrc capabilities)
pub fn install_asset_shell(
    asset_key: &str,
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    replay: &PlayReplayContext,
    metadata_cache: Option<Arc<Mutex<InternalVideoMetadata>>>,
    surface: &VideoSurface,
    frame_sink: &Arc<crate::playback::frame::FrameSink>,
    #[cfg(target_os = "android")] overlay_size_sync: Option<OverlaySizeSync>,
) -> Result<PipelineShell> {
    let (pipeline, video_sink, feed, orientation_filter) = build_asset_pipeline(
        asset_key,
        emitter,
        metadata_cache,
        frame_sink,
        #[cfg(target_os = "android")]
        overlay_size_sync,
    )?;
    let (bus_watch, position_source) = attach_gst_bus_handlers(
        &pipeline,
        emitter,
        looping,
        &replay.desired_playing,
        &replay.at_eos,
        &replay.running,
        &replay.rate,
        false,
        None,
        #[cfg(target_os = "ios")]
        Some(surface.ios_layer_bus_slot()),
    )?;
    Ok(PipelineShell {
        pipeline,
        video_sink,
        kind: SourceKind::Asset,
        is_playbin: false,
        asset_key: Some(asset_key.to_string()),
        appsrc_feed: Some(feed),
        bus_watch: Some(bus_watch),
        position_source: Some(position_source),
        orientation_filter: Some(orientation_filter),
    })
}
/// 拆除 shell：释放总线监听与 AppSrc feed，置 Null / Tears down shell: releases bus watch and AppSrc feed, sets Null.
pub fn teardown_shell(shell: &mut PipelineShell) {
    shell.bus_watch = None;
    shell.position_source = None;
    shell.appsrc_feed = None;
    shell.set_state_null();
}

/// 为 `prepare-window-handle` 安装总线 sync handler / Wires bus sync handler for `prepare-window-handle`.
///
/// # 参数 / Parameters
/// - `shell` — 已安装 shell / installed shell
/// - `overlay_handle` — 缓存的原生句柄 / cached native handle
/// - `overlay_sink`（macOS/iOS）— 可选 sink 槽 / optional sink slot
///
/// # 返回值 / Returns
/// - 无 / None
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - sync handler 在 Gst 线程 / sync handler on Gst thread
///
/// # 平台 / Platform
/// - macOS/iOS 传入 overlay sink 槽；其他平台直接绑定句柄 / platform-specific bind behavior
pub fn wire_overlay_sync(
    shell: &PipelineShell,
    overlay_handle: Arc<Mutex<Option<usize>>>,
    #[cfg(target_os = "ios")] overlay_sink: Option<Arc<Mutex<gst::Element>>>,
) {
    #[cfg(target_os = "ios")]
    attach_overlay_bus_sync_handler(&shell.pipeline, overlay_handle, overlay_sink);
    #[cfg(not(target_os = "ios"))]
    attach_overlay_bus_sync_handler(&shell.pipeline, overlay_handle);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_gst() {
        let _ = gst::init();
    }

    fn empty_shell(kind: SourceKind, asset_key: Option<String>) -> PipelineShell {
        init_gst();
        let pipeline = gst::Pipeline::new();
        PipelineShell {
            pipeline,
            video_sink: gst::ElementFactory::make("fakesink")
                .build()
                .expect("fakesink"),
            kind,
            is_playbin: kind == SourceKind::Uri,
            asset_key,
            appsrc_feed: None,
            bus_watch: None,
            position_source: None,
            orientation_filter: None,
        }
    }

    fn uri_shell(uri: Option<&str>) -> PipelineShell {
        init_gst();
        let playbin = gst::ElementFactory::make("playbin3")
            .build()
            .expect("playbin3");
        if let Some(uri) = uri {
            playbin.set_property("uri", uri);
        }
        let pipeline = playbin
            .dynamic_cast::<gst::Pipeline>()
            .expect("playbin3 pipeline");
        PipelineShell {
            pipeline,
            video_sink: gst::ElementFactory::make("fakesink")
                .build()
                .expect("fakesink"),
            kind: SourceKind::Uri,
            is_playbin: true,
            asset_key: None,
            appsrc_feed: None,
            bus_watch: None,
            position_source: None,
            orientation_filter: None,
        }
    }

    #[test]
    fn has_pending_media_asset_with_key() {
        let shell = empty_shell(SourceKind::Asset, Some("assets/sample.mp4".to_string()));
        assert!(shell.has_pending_media());
    }

    #[test]
    fn has_pending_media_asset_without_key() {
        let shell = empty_shell(SourceKind::Asset, None);
        assert!(!shell.has_pending_media());
    }

    #[test]
    fn has_pending_media_asset_empty_key() {
        let shell = empty_shell(SourceKind::Asset, Some(String::new()));
        assert!(!shell.has_pending_media());
    }

    #[test]
    fn has_pending_media_uri_unset() {
        let shell = uri_shell(None);
        assert!(!shell.has_pending_media());
    }

    #[test]
    fn has_pending_media_uri_set() {
        let shell = uri_shell(Some("https://example.com/video.mp4"));
        assert!(shell.has_pending_media());
    }

    #[test]
    fn source_kind_accessors() {
        let uri = empty_shell(SourceKind::Uri, None);
        assert!(uri.is_uri());
        assert_eq!(uri.source_kind(), SourceKind::Uri);
        assert!(uri.asset_key().is_none());

        let asset = empty_shell(SourceKind::Asset, Some("assets/x.mp4".to_string()));
        assert!(!asset.is_uri());
        assert_eq!(asset.asset_key(), Some("assets/x.mp4"));
    }

    #[test]
    fn snapshot_reflects_has_pending_media() {
        let shell = uri_shell(Some("file:///tmp/x.mp4"));
        let snap = shell.snapshot();
        assert!(snap.has_pending_media);
        assert_eq!(snap.current, gst::State::Null);
    }

    // Real playable pipeline whose current segment rate can be queried.
    fn playing_rate_shell() -> PipelineShell {
        init_gst();
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
        let shell = PipelineShell {
            pipeline,
            video_sink: sink,
            kind: SourceKind::Uri,
            is_playbin: false,
            asset_key: None,
            appsrc_feed: None,
            bus_watch: None,
            position_source: None,
            orientation_filter: None,
        };
        shell
            .set_state_sync(gst::State::Playing)
            .expect("to playing");
        shell
    }

    fn segment_rate(shell: &PipelineShell) -> f64 {
        let mut q = gst::query::Segment::new(gst::Format::Time);
        assert!(shell.pipeline.query(&mut q), "segment query failed");
        q.result().0
    }

    // Regression: `apply_playback_rate` must send a rate-bearing segment (so
    // scaletempo preserves pitch), not an INSTANT_RATE_CHANGE that leaves the
    // segment rate at 1.0.
    #[test]
    fn apply_playback_rate_sets_segment_rate() {
        let shell = playing_rate_shell();
        shell.apply_playback_rate(2.0).expect("apply rate");
        assert!((segment_rate(&shell) - 2.0).abs() < 1e-6);
        let _ = shell.set_state_null();
    }

    // Regression: EOS replay / loop must restart at the selected rate, not 1.0
    // (the old `seek_simple` reset the rate).
    #[test]
    fn seek_to_start_with_rate_preserves_rate() {
        let shell = playing_rate_shell();
        shell.seek_to_start_with_rate(2.0).expect("seek start rate");
        assert!((segment_rate(&shell) - 2.0).abs() < 1e-6);
        let _ = shell.set_state_null();
    }
}
