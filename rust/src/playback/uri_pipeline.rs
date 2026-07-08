use std::sync::Arc;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::playback::bus::Emitter;
use crate::playback::frame::FrameSink;
use crate::playback::gst::{create_platform_video_sink, InternalVideoMetadata};
#[cfg(target_os = "android")]
use crate::playback::sink::OverlaySizeSync;
use crate::playback::sink::{
    attach_video_probe, build_audio_sink_bin, build_text_sink_bin, configure_http_source,
};

/// Builds a `playbin3` pipeline for URI/network/file sources.
pub fn build_uri_playbin(
    emitter: &Arc<Mutex<Option<Emitter>>>,
    metadata_cache: Option<Arc<Mutex<InternalVideoMetadata>>>,
    frame_sink: &Arc<FrameSink>,
    #[cfg(target_os = "android")] overlay_size_sync: Option<OverlaySizeSync>,
) -> Result<(gst::Pipeline, gst::Element)> {
    let playbin = gst::ElementFactory::make("playbin3")
        .build()
        .map_err(|_| anyhow!("failed to create playbin3"))?;

    let video_sink = create_platform_video_sink(frame_sink)?;
    attach_video_probe(
        &video_sink,
        emitter.clone(),
        metadata_cache,
        #[cfg(target_os = "android")]
        overlay_size_sync,
    );

    playbin.set_property("video-sink", &video_sink);

    let audio_bin = build_audio_sink_bin()?;
    playbin.set_property("audio-sink", &audio_bin);

    if let Ok(text_bin) = build_text_sink_bin() {
        playbin.set_property("text-sink", &text_bin);
    }

    playbin.connect("source-setup", false, |values| {
        if let Ok(element) = values[1].get::<gst::Element>() {
            configure_http_source(&element);
        }
        None
    });
    playbin.connect("element-setup", false, |values| {
        if let Ok(element) = values[1].get::<gst::Element>() {
            configure_http_source(&element);
        }
        None
    });

    let pipeline = playbin
        .dynamic_cast::<gst::Pipeline>()
        .map_err(|_| anyhow!("playbin3 is not a pipeline"))?;
    Ok((pipeline, video_sink))
}
