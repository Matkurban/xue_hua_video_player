use std::sync::Arc;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video as gst_video;
use parking_lot::Mutex;

use crate::playback::bus::Emitter;
use crate::playback::gst::{expose_overlay, InternalVideoMetadata};

/// Configures an HTTP(S) source element for permissive TLS and a mobile user-agent.
pub fn configure_http_source(element: &gst::Element) {
    if element.find_property("ssl-strict").is_some() {
        element.set_property("ssl-strict", false);
    }
    if element.find_property("tls-validation-flags").is_some() {
        // GIO_TLS_CERTIFICATE_VALIDATE_ALL = 0x7f (permissive when combined with ssl-strict=false).
        element.set_property("tls-validation-flags", 0u32);
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
            log::warn!("scaletempo unavailable: playback speed may change pitch");
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

/// Builds a fakesink text bin so playbin exposes subtitle track metadata without rendering.
pub fn build_text_sink_bin() -> Result<gst::Bin> {
    let text_bin = gst::Bin::new();
    let fakesink = gst::ElementFactory::make("fakesink")
        .build()
        .map_err(|_| anyhow!("failed to create fakesink for text"))?;
    text_bin.add(&fakesink)?;
    let sink_pad = fakesink
        .static_pad("sink")
        .ok_or_else(|| anyhow!("fakesink has no sink pad"))?;
    let ghost = gst::GhostPad::with_target(&sink_pad)?;
    ghost.set_active(true)?;
    text_bin.add_pad(&ghost)?;
    Ok(text_bin)
}

/// Emits video size and metadata events when decoded dimensions change.
pub fn attach_video_probe(
    video_sink: &gst::Element,
    emitter: Arc<Mutex<Option<Emitter>>>,
    metadata_cache: Option<Arc<Mutex<InternalVideoMetadata>>>,
) {
    let sink_pad = match video_sink.static_pad("sink") {
        Some(pad) => pad,
        None => return,
    };
    let last_size = Arc::new(Mutex::new((0i32, 0i32)));
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
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
                            let meta = InternalVideoMetadata::from_video_info_and_caps(
                                &video_info,
                                Some(caps.caps()),
                            );
                            if let Some(cache) = metadata_cache.as_ref() {
                                *cache.lock() = meta.clone();
                            }
                            cb(PlayerEvent::metadata(meta));
                        }
                        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
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
