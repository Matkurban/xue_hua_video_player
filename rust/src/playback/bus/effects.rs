use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

#[cfg(target_os = "ios")]
use crate::playback::overlay::IosLayerBackend;
use crate::playback::tracks::{mark_selected_streams, update_cache_from_collection, TrackCache};
use crate::player_events::{PlayerEvent, PlayerState};

use super::reducer::{BusReplayPatch, BusSideEffect};

pub struct BusEffectContext<'a> {
    pub pipeline: &'a gst::Pipeline,
    pub msg: &'a gst::Message,
    pub track_cache: Option<&'a Arc<Mutex<TrackCache>>>,
    #[cfg(target_os = "ios")]
    pub ios_layer_bus: &'a Option<Arc<Mutex<Option<IosLayerBackend>>>>,
    pub emit: &'a mut dyn FnMut(PlayerEvent),
}

pub fn apply_bus_replay_patch(
    patch: BusReplayPatch,
    at_eos: &Arc<AtomicBool>,
    desired_playing: &Arc<AtomicBool>,
) {
    if let Some(value) = patch.at_eos {
        at_eos.store(value, Ordering::SeqCst);
    }
    if let Some(value) = patch.desired_playing {
        desired_playing.store(value, Ordering::SeqCst);
    }
}

pub fn apply_bus_side_effects(effects: &[BusSideEffect], ctx: &mut BusEffectContext<'_>) {
    for effect in effects {
        apply_bus_side_effect(effect, ctx);
    }
}

fn apply_bus_side_effect(effect: &BusSideEffect, ctx: &mut BusEffectContext<'_>) {
    match effect {
        BusSideEffect::EosLoopSeek => {
            let _ = ctx.pipeline.seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::ClockTime::ZERO,
            );
        }
        BusSideEffect::PausePipelineForBuffering => pause_pipeline_for_buffering(ctx.pipeline),
        BusSideEffect::ResumePipelineAfterBuffering => {
            resume_pipeline_after_buffering(ctx.pipeline, ctx.emit);
        }
        BusSideEffect::ClockLostRecover => clock_lost_recover(ctx.pipeline),
        #[cfg(target_os = "ios")]
        BusSideEffect::IosSetBufferingActive(active) => {
            ios_set_buffering_active(ctx.ios_layer_bus, *active);
        }
        #[cfg(target_os = "ios")]
        BusSideEffect::IosSetPendingPlayAfterOverlay => {
            ios_set_pending_play_after_overlay(ctx.ios_layer_bus);
        }
        #[cfg(target_os = "ios")]
        BusSideEffect::IosScheduleApply => ios_schedule_apply(ctx.ios_layer_bus),
        #[cfg(target_os = "ios")]
        BusSideEffect::IosScheduleAttach => ios_schedule_attach(ctx.ios_layer_bus),
        BusSideEffect::TrackCacheSyncFromCollection => {
            if let (Some(cache), gst::MessageView::StreamCollection(sc)) =
                (ctx.track_cache, ctx.msg.view())
            {
                update_cache_from_collection(&sc.stream_collection(), cache);
            }
        }
        BusSideEffect::TrackCacheMarkSelected => {
            if let (Some(cache), gst::MessageView::StreamsSelected(ss)) =
                (ctx.track_cache, ctx.msg.view())
            {
                mark_selected_streams(&ss, cache);
            }
        }
    }
}

fn pause_pipeline_for_buffering(pipeline: &gst::Pipeline) {
    #[cfg(target_os = "android")]
    if let Err(e) = pipeline.set_state(gst::State::Paused) {
        log::warn!("buffering set_state(Paused): {e}");
    }
    #[cfg(not(target_os = "android"))]
    if let Err(e) = crate::playback::shell::set_element_state_sync(pipeline, gst::State::Paused) {
        log::warn!("buffering set_state_sync(Paused): {e}");
    }
}

fn resume_pipeline_after_buffering(pipeline: &gst::Pipeline, emit: &mut dyn FnMut(PlayerEvent)) {
    let resume = {
        #[cfg(target_os = "android")]
        {
            pipeline.set_state(gst::State::Playing)
        }
        #[cfg(not(target_os = "android"))]
        {
            crate::playback::shell::set_element_state_sync(pipeline, gst::State::Playing)
        }
    };
    if let Err(e) = resume {
        log::warn!("buffering resume Playing: {e}");
    } else {
        emit(PlayerEvent::state(PlayerState::Playing));
    }
}

fn clock_lost_recover(pipeline: &gst::Pipeline) {
    #[cfg(target_os = "android")]
    {
        let _ = pipeline.set_state(gst::State::Paused);
        if let Err(e) = pipeline.set_state(gst::State::Playing) {
            log::warn!("clock-lost resume Playing: {e}");
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if let Err(e) = crate::playback::shell::set_element_state_sync(pipeline, gst::State::Paused)
        {
            log::warn!("clock-lost set_state_sync(Paused): {e}");
        }
        if let Err(e) =
            crate::playback::shell::set_element_state_sync(pipeline, gst::State::Playing)
        {
            log::warn!("clock-lost set_state_sync(Playing): {e}");
        }
    }
}

#[cfg(target_os = "ios")]
fn ios_with_hook(
    ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>,
    f: impl FnOnce(&IosLayerBackend),
) {
    if let Some(slot) = ios_layer_bus.as_ref() {
        if let Some(hook) = slot.lock().as_ref() {
            f(hook);
        }
    }
}

#[cfg(target_os = "ios")]
fn ios_set_pending_play_after_overlay(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>) {
    ios_with_hook(ios_layer_bus, |hook| hook.set_pending_play_after_overlay());
}

#[cfg(target_os = "ios")]
fn ios_set_buffering_active(
    ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>,
    active: bool,
) {
    ios_with_hook(ios_layer_bus, |hook| hook.set_buffering_active(active));
}

#[cfg(target_os = "ios")]
fn ios_schedule_apply(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>) {
    ios_with_hook(ios_layer_bus, |hook| hook.schedule_apply());
}

#[cfg(target_os = "ios")]
fn ios_schedule_attach(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>) {
    ios_with_hook(ios_layer_bus, |hook| hook.schedule_attach());
}
