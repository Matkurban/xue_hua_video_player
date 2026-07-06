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

use crate::player_events::{map_state, PlayerEvent, PlayerState};
use crate::playback::tracks::{mark_selected_streams, update_cache_from_collection, TrackCache};

pub type Emitter = Arc<dyn Fn(PlayerEvent) + Send + Sync>;

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
) -> Result<(gst::bus::BusWatchGuard, gst::glib::SourceId)> {
    let bus = pipeline
        .bus()
        .ok_or_else(|| anyhow!("pipeline has no bus"))?;
    let pipeline_bus = pipeline.clone();
    let pipeline_pos = pipeline.clone();
    let track_cache_bus = track_cache.clone();
    let emitter_bus = emitter.clone();
    let emitter_pos = emitter.clone();
    let looping = looping.clone();
    let desired_playing = desired_playing.clone();
    let at_eos = at_eos.clone();
    let running_bus = running.clone();
    let running_pos = running.clone();

    let bus_watch = bus
        .add_watch_local(move |_, msg| {
            if !running_bus.load(Ordering::SeqCst) {
                return gst::glib::ControlFlow::Break;
            }
            let emit = |event: PlayerEvent| {
                if let Some(cb) = emitter_bus.lock().as_ref() {
                    cb(event);
                }
            };
            use gst::MessageView;
            match msg.view() {
                MessageView::Eos(..) => {
                    if looping.load(Ordering::SeqCst) {
                        at_eos.store(false, Ordering::SeqCst);
                        let _ = pipeline_bus.seek_simple(
                            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                            gst::ClockTime::ZERO,
                        );
                    } else {
                        at_eos.store(true, Ordering::SeqCst);
                        emit(PlayerEvent::eos());
                        emit(PlayerEvent::state(PlayerState::Completed));
                    }
                }
                MessageView::Error(err) => {
                    log::error!(
                        "GStreamer error: {} ({:?})",
                        err.error(),
                        err.debug()
                    );
                    emit(PlayerEvent::error(format!(
                        "{} ({:?})",
                        err.error(),
                        err.debug()
                    )));
                    emit(PlayerEvent::state(PlayerState::Error));
                }
                MessageView::Buffering(b) => {
                    let percent = b.percent();
                    emit(PlayerEvent::buffering(percent));
                    if desired_playing.load(Ordering::SeqCst) {
                        if percent < 100 {
                            emit(PlayerEvent::state(PlayerState::Buffering));
                        }
                        let target = if percent < 100 {
                            gst::State::Paused
                        } else {
                            gst::State::Playing
                        };
                        if let Err(e) = pipeline_bus.set_state(target) {
                            log::warn!("buffering set_state({target:?}): {e}");
                        }
                        if percent >= 100 {
                            emit(PlayerEvent::state(PlayerState::Playing));
                        }
                    }
                }
                MessageView::DurationChanged(..) => {
                    if let Some(d) = pipeline_bus.query_duration::<gst::ClockTime>() {
                        emit(PlayerEvent::duration(d.mseconds() as i64));
                    }
                }
                MessageView::AsyncDone(..) => {
                    if desired_playing.load(Ordering::SeqCst) {
                        if let Some(p) = pipeline_bus.query_position::<gst::ClockTime>() {
                            emit(PlayerEvent::position(p.mseconds() as i64));
                        }
                    }
                }
                MessageView::StateChanged(sc) => {
                    if sc.src().map(|s| s == &pipeline_bus).unwrap_or(false) {
                        let current = sc.current();
                        if !(current == gst::State::Paused
                            && desired_playing.load(Ordering::SeqCst))
                        {
                            emit(PlayerEvent::state(map_state(current)));
                        }
                        if current == gst::State::Playing
                            && desired_playing.load(Ordering::SeqCst)
                        {
                            emit(PlayerEvent::buffering(100));
                        }
                        if current == gst::State::Paused
                            || current == gst::State::Playing
                        {
                            if let Some(d) = pipeline_bus.query_duration::<gst::ClockTime>() {
                                emit(PlayerEvent::duration(d.mseconds() as i64));
                            }
                        }
                    }
                }
                MessageView::StreamCollection(sc) if is_playbin => {
                    if let Some(cache) = track_cache_bus.as_ref() {
                        update_cache_from_collection(&sc.stream_collection(), cache);
                    }
                    emit(PlayerEvent::tracks_changed());
                }
                MessageView::StreamsSelected(ss) if is_playbin => {
                    if let Some(cache) = track_cache_bus.as_ref() {
                        mark_selected_streams(&ss, cache);
                    }
                    emit(PlayerEvent::tracks_changed());
                }
                _ => {}
            }
            gst::glib::ControlFlow::Continue
        })
        .map_err(|e| anyhow!("bus watch failed: {e}"))?;

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
