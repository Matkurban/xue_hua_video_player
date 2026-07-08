//! URI / 网络 / 文件 `playbin3` 管线构建 / URI/network/file `playbin3` pipeline builder.
//!
//! 为 [`crate::playback::shell::install_uri_shell`] 构建 `playbin3` 管线：含 `videoflip` 的
//! video-sink bin、音频 scaletempo bin、可选字幕 fakesink，以及 HTTP 源 TLS/UA 配置。
//!
//! Builds `playbin3` for [`crate::playback::shell::install_uri_shell`]: video-sink bin with
//! `videoflip`, audio scaletempo bin, optional subtitle fakesink, and HTTP source TLS/UA config.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::playback::bus::Emitter;
use crate::playback::frame::FrameSink;
use crate::playback::gst::{build_video_sink_bin, InternalVideoMetadata};
#[cfg(target_os = "android")]
use crate::playback::sink::OverlaySizeSync;
use crate::playback::sink::{
    attach_video_probe, build_audio_sink_bin, build_text_sink_bin, configure_http_source,
};

/// 为 URI/网络/文件源构建 `playbin3` 管线 / Builds a `playbin3` pipeline for URI/network/file sources.
///
/// # 返回值 / Returns
/// - 成功：`(Pipeline, inner_video_sink, orientation_videoflip)` / pipeline, appsink/overlay sink, videoflip
pub fn build_uri_playbin(
    emitter: &Arc<Mutex<Option<Emitter>>>,
    metadata_cache: Option<Arc<Mutex<InternalVideoMetadata>>>,
    frame_sink: &Arc<FrameSink>,
    #[cfg(target_os = "android")] overlay_size_sync: Option<OverlaySizeSync>,
) -> Result<(gst::Pipeline, gst::Element, gst::Element)> {
    let playbin = gst::ElementFactory::make("playbin3")
        .build()
        .map_err(|_| anyhow!("failed to create playbin3"))?;

    let (video_sink_bin, video_sink, orientation_filter) = build_video_sink_bin(frame_sink)?;
    attach_video_probe(
        &video_sink,
        emitter.clone(),
        metadata_cache,
        #[cfg(target_os = "android")]
        overlay_size_sync,
    );

    playbin.set_property("video-sink", &video_sink_bin);

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
    Ok((pipeline, video_sink, orientation_filter))
}
