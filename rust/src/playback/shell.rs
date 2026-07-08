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
use crate::playback::gst::apply_orientation_to_playbin;
use crate::playback::gst::attach_overlay_bus_sync_handler;
use crate::playback::gst::{
    expose_overlay, set_overlay_render_rectangle, set_overlay_window_handle,
    InternalAspectRatioMode, InternalVideoMetadata, InternalVideoOrientationConfig,
};
use crate::playback::overlay::PipelineSnapshot;
use crate::playback::replay::PlayReplayContext;
use crate::playback::surface::VideoSurface;
use crate::playback::tracks::{
    disable_subtitles_on_pipeline, select_track_on_pipeline, TrackCache,
};
use crate::playback::uri_pipeline::build_uri_playbin;
use crate::player_events::TrackType;

const DEFAULT_STATE_TIMEOUT: gst::ClockTime = gst::ClockTime::from_seconds(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Uri,
    Asset,
}

/// Shared pipeline shell: sinks, bus handlers, and overlay sync wiring.
pub struct PipelineShell {
    pipeline: gst::Pipeline,
    video_sink: gst::Element,
    kind: SourceKind,
    is_playbin: bool,
    asset_key: Option<String>,
    appsrc_feed: Option<Arc<AppSrcFeedState>>,
    bus_watch: Option<gst::bus::BusWatchGuard>,
    position_source: Option<gst::glib::SourceId>,
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

    /// True when the shell has a URI or asset key ready for preroll.
    ///
    /// An empty playbin (`SourceKind::Uri` with `uri` unset) returns `false` so
    /// early overlay bind does not panic or preroll before `load()`.
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

    pub fn seek_to_start(&self) -> Result<()> {
        self.pipeline
            .seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::ClockTime::ZERO,
            )
            .map_err(|e| anyhow!("seek to start before play: {e}"))
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

    pub fn apply_playback_rate(&self, rate: f64) -> Result<()> {
        self.pipeline
            .seek(
                rate,
                gst::SeekFlags::INSTANT_RATE_CHANGE,
                gst::SeekType::None,
                gst::ClockTime::ZERO,
                gst::SeekType::None,
                gst::ClockTime::ZERO,
            )
            .map_err(|e| anyhow!("apply playback rate failed: {e}"))
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

    pub fn apply_orientation(&self, config: InternalVideoOrientationConfig) -> Result<()> {
        apply_orientation_to_playbin(self.pipeline.upcast_ref::<gst::Element>(), config)
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
    }
}

/// Sets pipeline/element state and waits until the transition completes.
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

pub fn install_uri_shell(
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    replay: &PlayReplayContext,
    metadata_cache: Option<Arc<Mutex<InternalVideoMetadata>>>,
    track_cache: Option<Arc<Mutex<TrackCache>>>,
    surface: &VideoSurface,
) -> Result<PipelineShell> {
    let (pipeline, video_sink) = build_uri_playbin(emitter, metadata_cache)?;
    let (bus_watch, position_source) = attach_gst_bus_handlers(
        &pipeline,
        emitter,
        looping,
        &replay.desired_playing,
        &replay.at_eos,
        &replay.running,
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
    })
}

pub fn install_asset_shell(
    asset_key: &str,
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    replay: &PlayReplayContext,
    metadata_cache: Option<Arc<Mutex<InternalVideoMetadata>>>,
    surface: &VideoSurface,
) -> Result<PipelineShell> {
    let (pipeline, video_sink, feed) = build_asset_pipeline(asset_key, emitter, metadata_cache)?;
    let (bus_watch, position_source) = attach_gst_bus_handlers(
        &pipeline,
        emitter,
        looping,
        &replay.desired_playing,
        &replay.at_eos,
        &replay.running,
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
    })
}

pub fn teardown_shell(shell: &mut PipelineShell) {
    shell.bus_watch = None;
    shell.position_source = None;
    shell.appsrc_feed = None;
    shell.set_state_null();
}

pub fn wire_overlay_sync(
    shell: &PipelineShell,
    overlay_handle: Arc<Mutex<Option<usize>>>,
    #[cfg(any(target_os = "macos", target_os = "ios"))] overlay_sink: Option<
        Arc<Mutex<gst::Element>>,
    >,
) {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    attach_overlay_bus_sync_handler(&shell.pipeline, overlay_handle, overlay_sink);
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
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
}
