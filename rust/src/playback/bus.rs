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

use crate::playback::tracks::{mark_selected_streams, update_cache_from_collection, TrackCache};
#[cfg(target_os = "ios")]
use crate::playback::surface::IosLayerBusHook;
use crate::player_events::{map_state, PlayerEvent, PlayerState};

pub type Emitter = Arc<dyn Fn(PlayerEvent) + Send + Sync>;

#[cfg(target_os = "ios")]
fn ios_overlay_bound(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBusHook>>>>) -> bool {
    ios_layer_bus
        .as_ref()
        .and_then(|slot| slot.lock().as_ref().map(|hook| hook.is_overlay_bound()))
        .unwrap_or(false)
}

#[cfg(target_os = "ios")]
fn set_pending_play_after_overlay(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBusHook>>>>) {
    if let Some(slot) = ios_layer_bus.as_ref() {
        if let Some(hook) = slot.lock().as_ref() {
            hook.set_pending_play_after_overlay();
        }
    }
}

#[cfg(target_os = "ios")]
fn set_buffering_active(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBusHook>>>>, active: bool) {
    if let Some(slot) = ios_layer_bus.as_ref() {
        if let Some(hook) = slot.lock().as_ref() {
            hook.set_buffering_active(active);
        }
    }
}

#[cfg(target_os = "ios")]
fn schedule_ios_apply(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBusHook>>>>) {
    if let Some(slot) = ios_layer_bus.as_ref() {
        if let Some(hook) = slot.lock().as_ref() {
            hook.schedule_apply();
        }
    }
}

#[cfg(target_os = "ios")]
fn schedule_ios_attach(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBusHook>>>>) {
    if let Some(slot) = ios_layer_bus.as_ref() {
        if let Some(hook) = slot.lock().as_ref() {
            hook.schedule_attach();
        }
    }
}

#[cfg(target_os = "ios")]
fn try_mark_ios_video_ready(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBusHook>>>>) {
    if let Some(slot) = ios_layer_bus.as_ref() {
        if let Some(hook) = slot.lock().as_ref() {
            hook.try_mark_video_ready();
        }
    }
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
    #[cfg(target_os = "ios")] ios_layer_bus_slot: Option<Arc<Mutex<Option<IosLayerBusHook>>>>,
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
    #[cfg(target_os = "ios")]
    let ios_layer_bus = ios_layer_bus_slot;

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
                        desired_playing.store(false, Ordering::SeqCst);
                        emit(PlayerEvent::eos());
                        emit(PlayerEvent::state(PlayerState::Completed));
                    }
                }
                MessageView::Error(err) => {
                    let msg = format!("{} ({:?})", err.error(), err.debug());
                    log::error!("GStreamer error: {msg}");
                    #[cfg(target_os = "android")]
                    crate::diag::logcat_error(&format!("GStreamer error: {msg}"));
                    emit(PlayerEvent::error(msg.clone()));
                    emit(PlayerEvent::state(PlayerState::Error));
                }
                MessageView::Buffering(b) => {
                    let percent = b.percent();
                    emit(PlayerEvent::buffering(percent));
                    if !desired_playing.load(Ordering::SeqCst) {
                        return gst::glib::ControlFlow::Continue;
                    }
                    #[cfg(target_os = "ios")]
                    {
                        if percent < 100 {
                            emit(PlayerEvent::state(PlayerState::Buffering));
                            set_buffering_active(&ios_layer_bus, true);
                        } else {
                            set_buffering_active(&ios_layer_bus, false);
                            if !ios_overlay_bound(&ios_layer_bus) {
                                log::debug!(
                                    "gst: deferring buffering resume until iOS layer attached"
                                );
                                set_pending_play_after_overlay(&ios_layer_bus);
                            }
                        }
                        schedule_ios_apply(&ios_layer_bus);
                        if percent >= 100 && desired_playing.load(Ordering::SeqCst) {
                            emit(PlayerEvent::state(PlayerState::Playing));
                        }
                        return gst::glib::ControlFlow::Continue;
                    }
                    #[cfg(not(target_os = "ios"))]
                    {
                        if percent < 100 {
                            emit(PlayerEvent::state(PlayerState::Buffering));
                            #[cfg(target_os = "android")]
                            if let Err(e) = pipeline_bus.set_state(gst::State::Paused) {
                                log::warn!("buffering set_state(Paused): {e}");
                            }
                            #[cfg(not(target_os = "android"))]
                            if let Err(e) = crate::playback::state::set_state_sync(
                                &pipeline_bus,
                                gst::State::Paused,
                            ) {
                                log::warn!("buffering set_state_sync(Paused): {e}");
                            }
                        } else {
                            let resume = {
                                #[cfg(target_os = "android")]
                                {
                                    pipeline_bus.set_state(gst::State::Playing)
                                }
                                #[cfg(not(target_os = "android"))]
                                {
                                    crate::playback::state::set_state_sync(
                                        &pipeline_bus,
                                        gst::State::Playing,
                                    )
                                }
                            };
                            if let Err(e) = resume {
                                log::warn!("buffering resume Playing: {e}");
                            } else {
                                emit(PlayerEvent::state(PlayerState::Playing));
                            }
                        }
                    }
                }
                MessageView::ClockLost(..) => {
                    if desired_playing.load(Ordering::SeqCst) {
                        #[cfg(target_os = "ios")]
                        {
                            if !ios_overlay_bound(&ios_layer_bus) {
                                log::debug!(
                                    "gst: deferring clock-lost resume until iOS layer attached"
                                );
                                set_pending_play_after_overlay(&ios_layer_bus);
                            }
                            schedule_ios_apply(&ios_layer_bus);
                            return gst::glib::ControlFlow::Continue;
                        }
                        #[cfg(target_os = "android")]
                        {
                            let _ = pipeline_bus.set_state(gst::State::Paused);
                            if let Err(e) = pipeline_bus.set_state(gst::State::Playing) {
                                log::warn!("clock-lost resume Playing: {e}");
                            }
                        }
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        {
                            if let Err(e) =
                                crate::playback::state::set_state_sync(&pipeline_bus, gst::State::Paused)
                            {
                                log::warn!("clock-lost set_state_sync(Paused): {e}");
                            }
                            if let Err(e) = crate::playback::state::set_state_sync(
                                &pipeline_bus,
                                gst::State::Playing,
                            ) {
                                log::warn!("clock-lost set_state_sync(Playing): {e}");
                            }
                        }
                    }
                }
                MessageView::DurationChanged(..) => {
                    if let Some(d) = pipeline_bus.query_duration::<gst::ClockTime>() {
                        emit(PlayerEvent::duration(d.mseconds() as i64));
                    }
                }
                MessageView::AsyncDone(..) => {
                    #[cfg(target_os = "ios")]
                    {
                        schedule_ios_attach(&ios_layer_bus);
                        try_mark_ios_video_ready(&ios_layer_bus);
                    }
                    if desired_playing.load(Ordering::SeqCst) {
                        if let Some(p) = pipeline_bus.query_position::<gst::ClockTime>() {
                            emit(PlayerEvent::position(p.mseconds() as i64));
                        }
                    }
                }
                MessageView::StateChanged(sc) => {
                    if sc.src().map(|s| s == &pipeline_bus).unwrap_or(false) {
                        let current = sc.current();
                        #[cfg(target_os = "ios")]
                        if sc.old() == gst::State::Ready && current == gst::State::Paused {
                            schedule_ios_attach(&ios_layer_bus);
                            try_mark_ios_video_ready(&ios_layer_bus);
                        }
                        if !(current == gst::State::Paused
                            && desired_playing.load(Ordering::SeqCst))
                        {
                            emit(PlayerEvent::state(map_state(current)));
                        }
                        if current == gst::State::Playing && desired_playing.load(Ordering::SeqCst)
                        {
                            emit(PlayerEvent::buffering(100));
                        }
                        if current == gst::State::Paused || current == gst::State::Playing {
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
