//! Flutter 资产 AppSrc 管线构建 / Flutter bundle asset AppSrc pipeline builder.
//!
//! 为 [`crate::playback::shell::install_asset_shell`] 构建 AppSrc → decodebin 管线，
//! 从 Flutter asset bundle 分块喂入数据并动态链接音视频支路。
//!
//! Builds AppSrc → decodebin for [`crate::playback::shell::install_asset_shell`],
//! feeding data from the Flutter asset bundle in chunks and dynamically linking A/V branches.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::{AppSrc, AppSrcCallbacks};
use parking_lot::Mutex;

use crate::media::AppSrcFeedState;
use crate::playback::bus::Emitter;
use crate::playback::frame::FrameSink;
use crate::playback::gst::{
    create_platform_video_sink, make_orientation_element, InternalVideoMetadata,
};
#[cfg(target_os = "android")]
use crate::playback::gst::link_android_gl_video_branch;
#[cfg(target_os = "android")]
use crate::playback::sink::OverlaySizeSync;
use crate::playback::sink::{attach_video_probe, build_audio_sink_bin};

/// AppSrc 每次 `need-data` 读取的字节块大小 / Bytes read per AppSrc `need-data` callback.
const APPSRC_CHUNK: usize = 64 * 1024;

/// 为 Flutter bundle 资产构建 AppSrc → decodebin 管线 / Builds an AppSrc → decodebin pipeline for Flutter bundle assets.
///
/// 返回 `(Pipeline, video_sink, feed, orientation_filter)`，其中 `orientation_filter` 为视频支路旋转元素（Android：`gltransformation`；其他：`videoflip`）。
pub fn build_asset_pipeline(
    asset_key: &str,
    emitter: &Arc<Mutex<Option<Emitter>>>,
    metadata_cache: Option<Arc<Mutex<InternalVideoMetadata>>>,
    frame_sink: &Arc<FrameSink>,
    #[cfg(target_os = "android")] overlay_size_sync: Option<OverlaySizeSync>,
) -> Result<(
    gst::Pipeline,
    gst::Element,
    Arc<AppSrcFeedState>,
    gst::Element,
)> {
    let pipeline = gst::Pipeline::new();
    let video_sink = create_platform_video_sink(frame_sink)?;
    attach_video_probe(
        &video_sink,
        emitter.clone(),
        metadata_cache,
        #[cfg(target_os = "android")]
        overlay_size_sync,
    );
    let audio_bin = build_audio_sink_bin()?;
    let orientation_filter = make_orientation_element()?;

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
        &orientation_filter,
        &video_sink,
        audio_bin.upcast_ref::<gst::Element>(),
    ])?;
    gst::Element::link_many([&appsrc, &decodebin])?;

    let pipeline_weak = pipeline.downgrade();
    let video_sink_cb = video_sink.clone();
    let audio_bin_cb = audio_bin.clone();
    let orientation_cb = orientation_filter.clone();
    decodebin.connect_pad_added(move |_elem, src_pad| {
        let Some(pipeline) = pipeline_weak.upgrade() else {
            return;
        };
        if src_pad.is_linked() {
            return;
        }
        let caps = match src_pad
            .current_caps()
            .or_else(|| Some(src_pad.query_caps(None)))
        {
            Some(c) => c,
            None => return,
        };
        let structure = match caps.structure(0) {
            Some(s) => s,
            None => return,
        };
        let name = structure.name();
        let result = if name.starts_with("video/") {
            link_video_branch(src_pad, &pipeline, &video_sink_cb, &orientation_cb)
        } else if name.starts_with("audio/") {
            link_audio_branch(src_pad, &pipeline, &audio_bin_cb)
        } else {
            Ok(())
        };
        if let Err(e) = result {
            log::error!("decodebin pad link failed: {e:#}");
        }
    });

    Ok((pipeline, video_sink, feed, orientation_filter))
}

/// 为 AppSrc 注册 `need-data` 分块推送回调 / Wires AppSrc `need-data` chunk push callbacks.
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

/// 将 decodebin 视频 pad 链接到视频 sink 支路 / Links decodebin video pad to sink branch.
///
/// Android：`queue → glupload → glcolorconvert → gltransformation → glimagesink`
/// 其他平台：`queue → videoflip → videoconvert → video_sink`
fn link_video_branch(
    src_pad: &gst::Pad,
    pipeline: &gst::Pipeline,
    video_sink: &gst::Element,
    videoflip: &gst::Element,
) -> Result<()> {
    let queue = gst::ElementFactory::make("queue").build()?;
    pipeline.add(&queue)?;
    let sink_pad = queue
        .static_pad("sink")
        .ok_or_else(|| anyhow!("queue has no sink pad"))?;
    src_pad.link(&sink_pad)?;
    queue.sync_state_with_parent()?;

    #[cfg(target_os = "android")]
    {
        return link_android_gl_video_branch(queue.upcast_ref(), videoflip, video_sink);
    }
    #[cfg(not(target_os = "android"))]
    {
        let convert = gst::ElementFactory::make("videoconvert").build()?;
        pipeline.add(&convert)?;
        gst::Element::link_many([queue.upcast_ref(), videoflip, &convert, video_sink])?;
        for el in [videoflip, &convert, video_sink] {
            el.sync_state_with_parent()?;
        }
        Ok(())
    }
}

/// 将 decodebin 音频 pad 链接到 queue → convert → resample → audio bin / Links decodebin audio pad to audio bin.
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
    for el in [
        &queue,
        &convert,
        &resample,
        audio_bin.upcast_ref::<gst::Element>(),
    ] {
        el.sync_state_with_parent()?;
    }
    Ok(())
}
