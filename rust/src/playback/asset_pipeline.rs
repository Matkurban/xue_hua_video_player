use std::sync::Arc;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::{AppSrc, AppSrcCallbacks};
use parking_lot::Mutex;

use crate::media::AppSrcFeedState;
use crate::playback::bus::Emitter;
use crate::playback::sink::{attach_video_probe, build_audio_sink_bin};
use crate::video::{create_platform_video_sink, info::InternalVideoMetadata};

const APPSRC_CHUNK: usize = 64 * 1024;

/// Builds an AppSrc → decodebin pipeline for Flutter bundle assets.
pub fn build_asset_pipeline(
    asset_key: &str,
    emitter: &Arc<Mutex<Option<Emitter>>>,
    metadata_cache: Option<Arc<Mutex<InternalVideoMetadata>>>,
) -> Result<(gst::Pipeline, gst::Element, Arc<AppSrcFeedState>)> {
    let pipeline = gst::Pipeline::new();
    let video_sink = create_platform_video_sink()?;
    attach_video_probe(&video_sink, emitter.clone(), metadata_cache);
    let audio_bin = build_audio_sink_bin()?;

    let appsrc = gst::ElementFactory::make("appsrc")
        .name("src")
        .build()
        .map_err(|e| anyhow!("appsrc: {e}"))?;
    let decodebin = gst::ElementFactory::make("decodebin")
        .name("dec")
        .build()
        .map_err(|e| anyhow!("decodebin: {e}"))?;

    let feed = Arc::new(AppSrcFeedState::new(asset_key)?);
    wire_appsrc_callbacks(&appsrc, feed.clone())?;

    pipeline.add_many([
        &appsrc,
        &decodebin,
        &video_sink,
        audio_bin.upcast_ref::<gst::Element>(),
    ])?;
    gst::Element::link_many([&appsrc, &decodebin])?;

    let pipeline_weak = pipeline.downgrade();
    let video_sink_cb = video_sink.clone();
    let audio_bin_cb = audio_bin.clone();
    decodebin.connect_pad_added(move |_elem, src_pad| {
        let Some(pipeline) = pipeline_weak.upgrade() else {
            return;
        };
        if src_pad.is_linked() {
            return;
        }
        let caps = match src_pad.current_caps().or_else(|| Some(src_pad.query_caps(None))) {
            Some(c) => c,
            None => return,
        };
        let structure = match caps.structure(0) {
            Some(s) => s,
            None => return,
        };
        let name = structure.name();
        let result = if name.starts_with("video/") {
            link_video_branch(src_pad, &pipeline, &video_sink_cb)
        } else if name.starts_with("audio/") {
            link_audio_branch(src_pad, &pipeline, &audio_bin_cb)
        } else {
            Ok(())
        };
        if let Err(e) = result {
            log::error!("decodebin pad link failed: {e:#}");
        }
    });

    Ok((pipeline, video_sink, feed))
}

fn wire_appsrc_callbacks(appsrc_el: &gst::Element, feed: Arc<AppSrcFeedState>) -> Result<()> {
    let appsrc = appsrc_el
        .clone()
        .dynamic_cast::<AppSrc>()
        .map_err(|_| anyhow!("element is not AppSrc"))?;
    appsrc.set_format(gst::Format::Bytes);
    appsrc.set_is_live(false);
    appsrc.set_block(true);

    appsrc.set_callbacks(
        AppSrcCallbacks::builder()
            .need_data(move |src, _size| {
                let mut guard = match feed.source.lock() {
                    Ok(g) => g,
                    Err(_) => {
                        let _ = src.end_of_stream();
                        return;
                    }
                };
                let Ok((bytes, eos)) = guard.read_chunk(APPSRC_CHUNK) else {
                    let _ = src.end_of_stream();
                    return;
                };
                if bytes.is_empty() {
                    let _ = src.end_of_stream();
                    return;
                }
                if let Err(e) = src.push_buffer(gst::Buffer::from_slice(bytes)) {
                    log::warn!("AppSrc push_buffer: {e}");
                    return;
                }
                if eos {
                    let _ = src.end_of_stream();
                }
            })
            .build(),
    );
    Ok(())
}

fn link_video_branch(
    src_pad: &gst::Pad,
    pipeline: &gst::Pipeline,
    video_sink: &gst::Element,
) -> Result<()> {
    let queue = gst::ElementFactory::make("queue").build()?;
    let convert = gst::ElementFactory::make("videoconvert").build()?;
    pipeline.add_many([&queue, &convert])?;
    gst::Element::link_many([&queue, &convert, video_sink])?;
    let sink_pad = queue
        .static_pad("sink")
        .ok_or_else(|| anyhow!("queue has no sink pad"))?;
    src_pad.link(&sink_pad)?;
    for el in [&queue, &convert, video_sink] {
        el.sync_state_with_parent()?;
    }
    Ok(())
}

fn link_audio_branch(
    src_pad: &gst::Pad,
    pipeline: &gst::Pipeline,
    audio_bin: &gst::Bin,
) -> Result<()> {
    let queue = gst::ElementFactory::make("queue").build()?;
    let convert = gst::ElementFactory::make("audioconvert").build()?;
    let resample = gst::ElementFactory::make("audioresample").build()?;
    pipeline.add_many([&queue, &convert, &resample])?;
    gst::Element::link_many([&queue, &convert, &resample])?;
    let audio_sink = audio_bin
        .static_pad("sink")
        .ok_or_else(|| anyhow!("audio bin has no sink pad"))?;
    let resample_src = resample
        .static_pad("src")
        .ok_or_else(|| anyhow!("audioresample has no src pad"))?;
    resample_src.link(&audio_sink)?;
    let queue_sink = queue
        .static_pad("sink")
        .ok_or_else(|| anyhow!("queue has no sink pad"))?;
    src_pad.link(&queue_sink)?;
    for el in [&queue, &convert, &resample, audio_bin.upcast_ref::<gst::Element>()] {
        el.sync_state_with_parent()?;
    }
    Ok(())
}
