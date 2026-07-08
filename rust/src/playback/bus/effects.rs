//! 总线归约副作用执行层（effects 阶段）。
//!
//! 在 Gst 线程上应用 [`BusReplayPatch`] 与 [`BusSideEffect`]。**必须在 replay patch 之后、
//! 或与事件发送顺序配合**：replay patch 更新原子 replay 标志；副作用可异步改变 pipeline
//! 状态或 iOS overlay 钩子。
//!
//! # English
//!
//! Applies [`BusReplayPatch`] and [`BusSideEffect`] on the Gst thread after reduction.
//! Replay patches update atomic flags before events are emitted; side effects perform
//! imperative pipeline / overlay / track-cache work.

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

/// 执行 [`BusSideEffect`] 所需的 Gst 线程上下文。
///
/// # 字段
///
/// - `pipeline` — 目标 GStreamer pipeline
/// - `msg` — 原始 bus 消息（track cache 同步需读取 `StreamCollection` / `StreamsSelected` 载荷）
/// - `track_cache` — playbin 音轨缓存；非 playbin 时为 `None`
/// - `rate` — 当前播放速率（EOS 循环 seek 需携带，避免重置为 1.0）
/// - `ios_layer_bus`（iOS）— overlay 后端槽位
/// - `emit` — 副作用完成后可向 Dart 补发事件的回调（如 buffering 恢复 Playing）
///
/// # English
///
/// Gst-thread context for applying side effects: pipeline, original message, optional track
/// cache, playback rate, iOS overlay slot, and an emit callback for follow-up events.
pub struct BusEffectContext<'a> {
    pub pipeline: &'a gst::Pipeline,
    pub msg: &'a gst::Message,
    pub track_cache: Option<&'a Arc<Mutex<TrackCache>>>,
    /// 当前播放速率，EOS 循环 seek 时保持用户所选倍速。
    /// Current playback rate so EOS loop seek keeps the user-selected speed.
    pub rate: f64,
    #[cfg(target_os = "ios")]
    pub ios_layer_bus: &'a Option<Arc<Mutex<Option<IosLayerBackend>>>>,
    pub emit: &'a mut dyn FnMut(PlayerEvent),
}

/// 将 [`BusReplayPatch`] 写入原子 replay 标志。
///
/// 在 [`super::mod::attach_gst_bus_handlers`] 中于 **发送 `PlayerEvent` 之前** 调用，确保
/// 后续逻辑与 Dart 层读到一致的 `at_eos` / `desired_playing` 状态。
///
/// 仅更新 patch 中为 `Some(_)` 的字段；`None` 字段保持不变。
///
/// # English
///
/// Applies optional `at_eos` and `desired_playing` updates to atomic flags before events are
/// emitted. Fields set to `None` in the patch are left unchanged.
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

/// 按顺序执行一组 [`BusSideEffect`]。
///
/// # English
///
/// Sequentially applies each effect in the slice via [`apply_bus_side_effect`].
pub fn apply_bus_side_effects(effects: &[BusSideEffect], ctx: &mut BusEffectContext<'_>) {
    for effect in effects {
        apply_bus_side_effect(effect, ctx);
    }
}

/// 执行单条 [`BusSideEffect`] 的具体 GStreamer / iOS / track-cache 操作。
///
/// # English
///
/// Dispatches one side effect to the appropriate imperative handler.
fn apply_bus_side_effect(effect: &BusSideEffect, ctx: &mut BusEffectContext<'_>) {
    match effect {
        BusSideEffect::EosLoopSeek => {
            // Seek to start carrying the current rate so looping keeps the
            // user-selected speed (seek_simple would reset the rate to 1.0),
            // and scaletempo receives a rate-bearing segment (pitch preserved).
            let _ = ctx.pipeline.seek(
                ctx.rate,
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::SeekType::Set,
                gst::ClockTime::ZERO,
                gst::SeekType::None,
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

/// 缓冲不足时将 pipeline 设为 Paused（非阻塞 `set_state`）。
///
/// 禁止在 bus watch 中调用 `set_state_sync`，否则会阻塞 xhvp-gst MainLoop。
///
/// # English
///
/// Non-blocking pause for network buffering; never uses `set_state_sync` on the bus watch.
fn pause_pipeline_for_buffering(pipeline: &gst::Pipeline) {
    // Never call set_state_sync from the bus watch — it blocks the xhvp-gst MainLoop.
    if let Err(e) = pipeline.set_state(gst::State::Paused) {
        log::warn!("buffering set_state(Paused): {e}");
    }
}

/// 缓冲完成后恢复 Playing，并通过 `emit` 通知 Dart。
///
/// # English
///
/// Resumes playback after buffering and emits `PlayerState::Playing` on success.
fn resume_pipeline_after_buffering(pipeline: &gst::Pipeline, emit: &mut dyn FnMut(PlayerEvent)) {
    if let Err(e) = pipeline.set_state(gst::State::Playing) {
        log::warn!("buffering resume Playing: {e}");
    } else {
        emit(PlayerEvent::state(PlayerState::Playing));
    }
}

/// 时钟丢失恢复：Paused → Playing 双步切换。
///
/// # English
///
/// Recovers from clock loss by pausing then resuming the pipeline.
fn clock_lost_recover(pipeline: &gst::Pipeline) {
    if let Err(e) = pipeline.set_state(gst::State::Paused) {
        log::warn!("clock-lost set_state(Paused): {e}");
    }
    if let Err(e) = pipeline.set_state(gst::State::Playing) {
        log::warn!("clock-lost resume Playing: {e}");
    }
}

/// 在 iOS overlay 后端存在时执行闭包。
///
/// # English
///
/// Runs `f` on the installed iOS layer backend when present.
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

/// 标记 overlay 就绪后需补发 play。
///
/// # English
///
/// Sets pending-play-after-overlay on the iOS layer backend.
#[cfg(target_os = "ios")]
fn ios_set_pending_play_after_overlay(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>) {
    ios_with_hook(ios_layer_bus, |hook| hook.set_pending_play_after_overlay());
}

/// 设置 iOS 缓冲活跃标志。
///
/// # English
///
/// Updates iOS buffering-active state on the layer backend.
#[cfg(target_os = "ios")]
fn ios_set_buffering_active(
    ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>,
    active: bool,
) {
    ios_with_hook(ios_layer_bus, |hook| hook.set_buffering_active(active));
}

/// 调度 iOS overlay 状态 apply。
///
/// # English
///
/// Schedules an iOS overlay apply on the layer backend.
#[cfg(target_os = "ios")]
fn ios_schedule_apply(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>) {
    ios_with_hook(ios_layer_bus, |hook| hook.schedule_apply());
}

/// 调度 iOS overlay attach。
///
/// # English
///
/// Schedules an iOS overlay attach on the layer backend.
#[cfg(target_os = "ios")]
fn ios_schedule_attach(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>) {
    ios_with_hook(ios_layer_bus, |hook| hook.schedule_attach());
}
