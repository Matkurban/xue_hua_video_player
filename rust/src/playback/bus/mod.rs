mod effects;
mod parse;
mod reducer;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::glib::source::{self, Priority};
use gstreamer::prelude::*;
use parking_lot::Mutex;

#[cfg(target_os = "ios")]
use crate::playback::overlay::IosLayerBackend;
use crate::playback::tracks::TrackCache;
use crate::player_events::PlayerEvent;

use reducer::BusSnapshot;

pub type Emitter = Arc<dyn Fn(PlayerEvent) + Send + Sync>;

#[cfg(target_os = "ios")]
fn ios_overlay_bound(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>) -> bool {
    ios_layer_bus
        .as_ref()
        .and_then(|slot| slot.lock().as_ref().map(|hook| hook.is_overlay_bound()))
        .unwrap_or(false)
}

/// Installs a bus watch and position polling timer on the Gst thread's MainContext.
pub fn attach_gst_bus_handlers(
    pipeline: &gst::Pipeline,
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    desired_playing: &Arc<AtomicBool>,
    at_eos: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    is_playbin: bool,
    track_cache: Option<Arc<Mutex<TrackCache>>>,
    #[cfg(target_os = "ios")] ios_layer_bus_slot: Option<Arc<Mutex<Option<IosLayerBackend>>>>,
) -> Result<(gst::bus::BusWatchGuard, gst::glib::SourceId)> {
    let bus = pipeline
        .bus()
        .ok_or_else(|| anyhow!("pipeline has no bus"))?;
    let pipeline_bus = pipeline.clone();
    let track_cache_bus = track_cache.clone();
    let emitter_bus = emitter.clone();
    let emitter_pos = emitter.clone();
    let looping = looping.clone();
    let desired_playing_bus = desired_playing.clone();
    let at_eos_bus = at_eos.clone();
    let running_bus = running.clone();
    let running_pos = running.clone();
    #[cfg(target_os = "ios")]
    let ios_layer_bus = ios_layer_bus_slot;

    let bus_watch = bus
        .add_watch_local(move |_, msg| {
            if !running_bus.load(Ordering::SeqCst) {
                return gst::glib::ControlFlow::Break;
            }

            let Some(parsed) = parse::parse_bus_message(msg, &pipeline_bus) else {
                return gst::glib::ControlFlow::Continue;
            };

            if matches!(&parsed, reducer::BusMessage::Error { .. }) {
                if let reducer::BusMessage::Error { message } = &parsed {
                    log::error!("GStreamer error: {message}");
                    #[cfg(target_os = "android")]
                    crate::diag::logcat_error(&format!("GStreamer error: {message}"));
                }
            }

            let snapshot = BusSnapshot::new(
                desired_playing_bus.load(Ordering::SeqCst),
                looping.load(Ordering::SeqCst),
                is_playbin,
                #[cfg(target_os = "ios")]
                ios_overlay_bound(&ios_layer_bus),
                #[cfg(not(target_os = "ios"))]
                false,
            );

            let reduction = reducer::reduce_bus_message(parsed, snapshot);

            effects::apply_bus_replay_patch(
                reduction.replay_patch,
                &at_eos_bus,
                &desired_playing_bus,
            );

            let mut emit_to_dart = |event: PlayerEvent| {
                if let Some(cb) = emitter_bus.lock().as_ref() {
                    cb(event);
                }
            };

            for event in reduction.events {
                emit_to_dart(event);
            }

            let mut effect_ctx = effects::BusEffectContext {
                pipeline: &pipeline_bus,
                msg,
                track_cache: track_cache_bus.as_ref(),
                #[cfg(target_os = "ios")]
                ios_layer_bus: &ios_layer_bus,
                emit: &mut emit_to_dart,
            };
            effects::apply_bus_side_effects(&reduction.effects, &mut effect_ctx);

            gst::glib::ControlFlow::Continue
        })
        .map_err(|e| anyhow!("bus watch failed: {e}"))?;

    let pipeline_pos = pipeline.clone();
    let ctx = crate::gst_runtime::gst_main_context()?.clone();
    let position_source = source::timeout_source_new(
        Duration::from_millis(200),
        Some("xhvp-position"),
        Priority::DEFAULT,
        move || {
            if !running_pos.load(Ordering::SeqCst) {
                return gst::glib::ControlFlow::Break;
            }
            let (_, current, _) = pipeline_pos.state(gst::ClockTime::ZERO);
            if current != gst::State::Playing && current != gst::State::Paused {
                return gst::glib::ControlFlow::Continue;
            }
            if let Some(cb) = emitter_pos.lock().as_ref() {
                if let Some(p) = pipeline_pos.query_position::<gst::ClockTime>() {
                    cb(PlayerEvent::position(p.mseconds() as i64));
                }
            }
            gst::glib::ControlFlow::Continue
        },
    )
    .attach(Some(&ctx));

    Ok((bus_watch, position_source))
}
