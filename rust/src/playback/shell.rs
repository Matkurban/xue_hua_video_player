use std::sync::{
    atomic::AtomicBool,
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::media::AppSrcFeedState;
use crate::playback::asset_pipeline::build_asset_pipeline;
use crate::playback::bus::{attach_gst_bus_handlers, Emitter};
use crate::playback::capabilities::PipelineCapabilities;
#[cfg(target_os = "ios")]
use crate::playback::surface::IosLayerBusHook;
use crate::playback::tracks::TrackCache;
use crate::playback::uri_pipeline::build_uri_playbin;
use crate::video::attach_overlay_bus_sync_handler;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Uri,
    Asset,
}

/// Shared pipeline shell: sinks, bus handlers, and overlay sync wiring.
pub struct PipelineShell {
    pub pipeline: gst::Pipeline,
    pub video_sink: gst::Element,
    pub kind: SourceKind,
    pub is_playbin: bool,
    pub asset_key: Option<String>,
    pub appsrc_feed: Option<Arc<AppSrcFeedState>>,
    pub bus_watch: Option<gst::bus::BusWatchGuard>,
    pub position_source: Option<gst::glib::SourceId>,
}

impl PipelineShell {
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
}

pub fn install_uri_shell(
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    desired_playing: &Arc<AtomicBool>,
    at_eos: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    metadata_cache: Option<Arc<Mutex<crate::video::info::InternalVideoMetadata>>>,
    track_cache: Option<Arc<Mutex<TrackCache>>>,
    #[cfg(target_os = "ios")] ios_layer_bus_slot: Option<&Arc<Mutex<Option<IosLayerBusHook>>>>,
) -> Result<PipelineShell> {
    let (pipeline, video_sink) = build_uri_playbin(emitter, metadata_cache)?;
    let (bus_watch, position_source) = attach_gst_bus_handlers(
        &pipeline,
        emitter,
        looping,
        desired_playing,
        at_eos,
        running,
        true,
        track_cache,
        #[cfg(target_os = "ios")]
        ios_layer_bus_slot.cloned(),
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
    desired_playing: &Arc<AtomicBool>,
    at_eos: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    metadata_cache: Option<Arc<Mutex<crate::video::info::InternalVideoMetadata>>>,
    #[cfg(target_os = "ios")] ios_layer_bus_slot: Option<&Arc<Mutex<Option<IosLayerBusHook>>>>,
) -> Result<PipelineShell> {
    let (pipeline, video_sink, feed) = build_asset_pipeline(asset_key, emitter, metadata_cache)?;
    let (bus_watch, position_source) = attach_gst_bus_handlers(
        &pipeline,
        emitter,
        looping,
        desired_playing,
        at_eos,
        running,
        false,
        None,
        #[cfg(target_os = "ios")]
        ios_layer_bus_slot.cloned(),
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
    let _ = shell.pipeline.set_state(gst::State::Null);
}

pub fn wire_overlay_sync(
    shell: &PipelineShell,
    overlay_handle: Arc<Mutex<Option<usize>>>,
    #[cfg(any(target_os = "macos", target_os = "ios"))] overlay_sink: Option<
        Arc<Mutex<gst::Element>>,
    >,
) {
    attach_overlay_bus_sync_handler(&shell.pipeline, overlay_handle, overlay_sink);
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
}
