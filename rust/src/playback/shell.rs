use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::media::AppSrcFeedState;
use crate::playback::bus::{attach_gst_bus_handlers, Emitter};
use crate::playback::asset_pipeline::build_asset_pipeline;
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
    pub appsrc_feed: Option<Arc<AppSrcFeedState>>,
    pub bus_watch: Option<gst::bus::BusWatchGuard>,
    pub position_source: Option<gst::glib::SourceId>,
}

pub fn install_uri_shell(
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    desired_playing: &Arc<AtomicBool>,
    at_eos: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    metadata_cache: Option<Arc<Mutex<crate::video::info::InternalVideoMetadata>>>,
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
    )?;
    Ok(PipelineShell {
        pipeline,
        video_sink,
        kind: SourceKind::Uri,
        is_playbin: true,
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
    )?;
    Ok(PipelineShell {
        pipeline,
        video_sink,
        kind: SourceKind::Asset,
        is_playbin: false,
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

pub fn wire_overlay_sync(shell: &PipelineShell, overlay_handle: Arc<Mutex<Option<usize>>>) {
    attach_overlay_bus_sync_handler(&shell.pipeline, overlay_handle);
}
