use std::sync::Arc;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video as gst_video;
use parking_lot::Mutex;

use crate::gst_bus::Emitter;
use crate::platform_overlay::create_platform_video_sink;
#[cfg(not(target_os = "macos"))]
use crate::platform_overlay::expose_overlay;

/// Configures an HTTP(S) source element for permissive TLS and a mobile user-agent.
pub fn configure_http_source(element: &gst::Element) {
    if element.find_property("ssl-strict").is_some() {
        element.set_property("ssl-strict", false);
    }
    if element.find_property("user-agent").is_some() {
        element.set_property(
            "user-agent",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148",
        );
    }
}

/// Builds an audio sink bin with optional `scaletempo` for pitch-preserving rate changes.
pub fn build_audio_sink_bin() -> Result<gst::Bin> {
    let audio_bin = gst::Bin::new();
    let audiosink = gst::ElementFactory::make("autoaudiosink")
        .build()
        .map_err(|_| anyhow!("failed to create autoaudiosink"))?;

    let head = match (
        gst::ElementFactory::make("scaletempo").build(),
        gst::ElementFactory::make("audioconvert").build(),
        gst::ElementFactory::make("audioresample").build(),
    ) {
        (Ok(scaletempo), Ok(audioconvert), Ok(audioresample)) => {
            audio_bin.add(&scaletempo)?;
            audio_bin.add(&audioconvert)?;
            audio_bin.add(&audioresample)?;
            audio_bin.add(&audiosink)?;
            scaletempo.link(&audioconvert)?;
            audioconvert.link(&audioresample)?;
            audioresample.link(&audiosink)?;
            scaletempo
        }
        _ => {
            log::warn!(
                "scaletempo unavailable: playback speed may change pitch"
            );
            audio_bin.add(&audiosink)?;
            audiosink
        }
    };

    let sink_pad = head
        .static_pad("sink")
        .ok_or_else(|| anyhow!("audio sink head has no sink pad"))?;
    let ghost = gst::GhostPad::with_target(&sink_pad)?;
    ghost.set_active(true)?;
    audio_bin.add_pad(&ghost)?;

    Ok(audio_bin)
}

/// Emits [`crate::player_events::PlayerEvent::video_size`] when decoded dimensions change.
pub fn attach_video_size_probe(video_sink: &gst::Element, emitter: Arc<Mutex<Option<Emitter>>>) {
    let sink_pad = match video_sink.static_pad("sink") {
        Some(pad) => pad,
        None => return,
    };
    let last_size = Arc::new(Mutex::new((0i32, 0i32)));
    #[cfg(not(target_os = "macos"))]
    let sink_for_expose = video_sink.clone();
    sink_pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
        if let Some(gst::PadProbeData::Event(ref ev)) = info.data {
            if let gst::EventView::Caps(caps) = ev.view() {
                if let Ok(video_info) = gst_video::VideoInfo::from_caps(caps.caps()) {
                    let width = video_info.width() as i32;
                    let height = video_info.height() as i32;
                    let mut ls = last_size.lock();
                    if *ls != (width, height) {
                        let first = ls.0 == 0 && ls.1 == 0;
                        *ls = (width, height);
                        if let Some(cb) = emitter.lock().as_ref() {
                            use crate::player_events::PlayerEvent;
                            cb(PlayerEvent::video_size(width, height));
                        }
                        #[cfg(not(target_os = "macos"))]
                        if first && width > 0 && height > 0 {
                            expose_overlay(&sink_for_expose);
                        }
                    }
                }
            }
        }
        gst::PadProbeReturn::Ok
    });
}

/// Builds a `playbin3` pipeline for URI/network/file sources.
pub fn build_uri_pipeline(emitter: &Arc<Mutex<Option<Emitter>>>) -> Result<(gst::Pipeline, gst::Element)> {
    let playbin = gst::ElementFactory::make("playbin3")
        .build()
        .map_err(|_| anyhow!("failed to create playbin3"))?;

    let video_sink = create_platform_video_sink()?;
    attach_video_size_probe(&video_sink, emitter.clone());

    playbin.set_property("video-sink", &video_sink);

    let audio_bin = build_audio_sink_bin()?;
    playbin.set_property("audio-sink", &audio_bin);

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
