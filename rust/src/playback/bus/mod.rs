//! GStreamer 总线（bus）消息处理模块。
//!
//! 本模块将 GStreamer 总线回调重构为 **纯归约（pure reduction）** 流水线，便于单元测试且与
//! GStreamer 类型解耦：
//!
//! ```text
//! gst::Message
//!     → parse_bus_message          （parse.rs：GStreamer → BusMessage）
//!     → reduce_bus_message         （reducer.rs：BusMessage + BusSnapshot → BusReduction）
//!     → apply_bus_replay_patch     （effects.rs：先更新原子 replay 标志）
//!     → emit PlayerEvent           （向 Dart 层发送事件）
//!     → apply_bus_side_effects     （effects.rs：在 Gst 线程执行命令式副作用）
//! ```
//!
//! [`BusReduction`] 包含三部分输出：
//! - **`events`** — 发送给 Flutter/Dart 的 [`PlayerEvent`]
//! - **`effects`** — 需在 Gst 线程执行的 [`BusSideEffect`]（如 seek、暂停/恢复 pipeline）
//! - **`replay_patch`** — 在发送事件**之前**写入的 [`BusReplayPatch`]（`at_eos`、`desired_playing`）
//!
//! 入口函数 [`attach_gst_bus_handlers`] 在 Gst 线程上注册 bus 处理与位置轮询。
//!
//! # English
//!
//! GStreamer bus message handling via a **pure reduction** pipeline decoupled from GStreamer
//! types for testability:
//!
//! `parse` → `reduce_bus_message(BusMessage, BusSnapshot)` → `events` + `BusSideEffect` +
//! `BusReplayPatch` → replay patch first, then emit events, then apply side effects on the Gst
//! thread. See [`attach_gst_bus_handlers`] for wiring.

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
#[cfg(not(target_os = "android"))]
use gstreamer::glib::source::{self, Priority};
use gstreamer::prelude::*;
use parking_lot::Mutex;

#[cfg(target_os = "ios")]
use crate::playback::overlay::IosLayerBackend;
use crate::playback::tracks::TrackCache;
use crate::player_events::PlayerEvent;

use reducer::BusSnapshot;

/// 向 Dart 层发送 [`PlayerEvent`] 的线程安全回调类型。
///
/// 由 `Arc<Mutex<Option<Emitter>>>` 持有，允许在 pipeline 生命周期内替换或清空 emitter。
///
/// # English
///
/// Thread-safe callback type for delivering [`PlayerEvent`] values to the Dart layer.
pub type Emitter = Arc<dyn Fn(PlayerEvent) + Send + Sync>;

#[cfg(not(target_os = "android"))]
pub type BusWatchHandles = (gst::glib::SourceId, gst::glib::SourceId);

#[cfg(target_os = "android")]
pub type BusWatchHandles = (
    crate::gst::BusPollToken,
    crate::gst::PositionPollToken,
);

/// Shared bus handler state for GLib watch callbacks and Android bus polling.
pub(crate) struct BusHandlerState {
    pub pipeline: gst::Pipeline,
    pub emitter: Arc<Mutex<Option<Emitter>>>,
    pub looping: Arc<AtomicBool>,
    pub desired_playing: Arc<AtomicBool>,
    pub at_eos: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
    pub rate: Arc<Mutex<f64>>,
    pub is_playbin: bool,
    pub track_cache: Option<Arc<Mutex<TrackCache>>>,
    #[cfg(target_os = "ios")]
    pub ios_layer_bus: Option<Arc<Mutex<Option<IosLayerBackend>>>>,
}

impl BusHandlerState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        pipeline: gst::Pipeline,
        emitter: Arc<Mutex<Option<Emitter>>>,
        looping: Arc<AtomicBool>,
        desired_playing: Arc<AtomicBool>,
        at_eos: Arc<AtomicBool>,
        running: Arc<AtomicBool>,
        rate: Arc<Mutex<f64>>,
        is_playbin: bool,
        track_cache: Option<Arc<Mutex<TrackCache>>>,
        #[cfg(target_os = "ios")] ios_layer_bus: Option<Arc<Mutex<Option<IosLayerBackend>>>>,
    ) -> Self {
        Self {
            pipeline,
            emitter,
            looping,
            desired_playing,
            at_eos,
            running,
            rate,
            is_playbin,
            track_cache,
            #[cfg(target_os = "ios")]
            ios_layer_bus,
        }
    }
}

/// 判断 iOS 视频 overlay 是否已绑定，用于 [`BusSnapshot::overlay_bound`]。
///
/// 当前 texture/appsink 路径无需 CALayer attach，恒返回 `true`（与 `overlay_ready_for_play`
/// 行为一致）。若未来恢复 CALayer 路径，应在此查询 `IosLayerBackend` 的实际绑定状态。
///
/// # English
///
/// Whether the iOS video overlay is bound, fed into [`BusSnapshot::overlay_bound`]. The
/// texture/appsink path always returns `true` (matches `overlay_ready_for_play`).
#[cfg(target_os = "ios")]
fn ios_overlay_bound(ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>) -> bool {
    // Texture/appsink path: no CALayer attach — always ready (matches overlay_ready_for_play).
    true
}

/// Dispatches one GStreamer bus message through the parse → reduce → effects pipeline.
pub(crate) fn dispatch_gst_bus_message(state: &BusHandlerState, msg: &gst::Message) {
    if !state.running.load(Ordering::SeqCst) {
        return;
    }

    let Some(parsed) = parse::parse_bus_message(msg, &state.pipeline) else {
        return;
    };

    if matches!(&parsed, reducer::BusMessage::Error { .. }) {
        if let reducer::BusMessage::Error { message } = &parsed {
            log::error!("GStreamer error: {message}");
            #[cfg(target_os = "android")]
            crate::diag::logcat_error(&format!("GStreamer error: {message}"));
        }
    }

    let snapshot = BusSnapshot::new(
        state.desired_playing.load(Ordering::SeqCst),
        state.looping.load(Ordering::SeqCst),
        state.is_playbin,
        #[cfg(target_os = "ios")]
        ios_overlay_bound(&state.ios_layer_bus),
        #[cfg(not(target_os = "ios"))]
        false,
    );

    let reduction = reducer::reduce_bus_message(parsed, snapshot);

    effects::apply_bus_replay_patch(
        reduction.replay_patch,
        &state.at_eos,
        &state.desired_playing,
    );

    let mut emit_to_dart = |event: PlayerEvent| {
        if let Some(cb) = state.emitter.lock().as_ref() {
            cb(event);
        }
    };

    for event in reduction.events {
        emit_to_dart(event);
    }

    let mut effect_ctx = effects::BusEffectContext {
        pipeline: &state.pipeline,
        msg,
        track_cache: state.track_cache.as_ref(),
        rate: *state.rate.lock(),
        #[cfg(target_os = "ios")]
        ios_layer_bus: &state.ios_layer_bus,
        emit: &mut emit_to_dart,
    };
    effects::apply_bus_side_effects(&reduction.effects, &mut effect_ctx);
}

/// 在 Gst 线程上安装 bus 处理与位置轮询。
///
/// 每条 bus 消息的处理顺序（与归约架构一致）：
///
/// 1. [`parse::parse_bus_message`] — 将 `gst::Message` 解析为 [`reducer::BusMessage`]
/// 2. 从原子变量构建 [`BusSnapshot`]
/// 3. [`reducer::reduce_bus_message`] — 纯函数归约，产出 events / effects / replay_patch
/// 4. [`effects::apply_bus_replay_patch`] — **先于**事件发送，更新 `at_eos` 与 `desired_playing`
/// 5. 遍历 `reduction.events` 调用 emitter
/// 6. [`effects::apply_bus_side_effects`] — 在 Gst 线程执行 pipeline seek、状态切换等副作用
///
/// 位置轮询独立于 bus，每 200 ms 在 Playing/Paused 状态下查询 position 并 emit。
///
/// # English
///
/// Installs bus handling and a 200 ms position poll on the Gst thread. Each message follows
/// the parse → snapshot → reduce → replay patch → emit events → apply side effects pipeline.
#[allow(clippy::too_many_arguments)]
pub fn attach_gst_bus_handlers(
    pipeline: &gst::Pipeline,
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    desired_playing: &Arc<AtomicBool>,
    at_eos: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    rate: &Arc<Mutex<f64>>,
    is_playbin: bool,
    track_cache: Option<Arc<Mutex<TrackCache>>>,
    #[cfg(target_os = "ios")] ios_layer_bus_slot: Option<Arc<Mutex<Option<IosLayerBackend>>>>,
) -> Result<BusWatchHandles> {
    let bus = pipeline
        .bus()
        .ok_or_else(|| anyhow!("pipeline has no bus"))?;

    let state = BusHandlerState::new(
        pipeline.clone(),
        emitter.clone(),
        looping.clone(),
        desired_playing.clone(),
        at_eos.clone(),
        running.clone(),
        rate.clone(),
        is_playbin,
        track_cache,
        #[cfg(target_os = "ios")]
        ios_layer_bus_slot,
    );

    #[cfg(target_os = "android")]
    {
        return Ok(crate::gst::android_runtime::register_bus_handlers(
            bus,
            state,
            pipeline.clone(),
            emitter.clone(),
            running.clone(),
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        let ctx = crate::gst::gst_main_context()?.clone();
        let pipeline_bus = pipeline.clone();
        let state_bus = state.clone_for_watch();
        let emitter_pos = emitter.clone();
        let running_pos = running.clone();

        let bus_source = bus.create_watch(
            Some("xhvp-bus"),
            Priority::DEFAULT,
            move |_, msg| {
                dispatch_gst_bus_message(&state_bus, msg);
                gst::glib::ControlFlow::Continue
            },
        );
        let bus_source_id = bus_source.attach(Some(&ctx));

        let pipeline_pos = pipeline_bus;
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

        Ok((bus_source_id, position_source))
    }
}

#[cfg(not(target_os = "android"))]
impl BusHandlerState {
    fn clone_for_watch(&self) -> Self {
        Self {
            pipeline: self.pipeline.clone(),
            emitter: self.emitter.clone(),
            looping: self.looping.clone(),
            desired_playing: self.desired_playing.clone(),
            at_eos: self.at_eos.clone(),
            running: self.running.clone(),
            rate: self.rate.clone(),
            is_playbin: self.is_playbin,
            track_cache: self.track_cache.clone(),
            #[cfg(target_os = "ios")]
            ios_layer_bus: self.ios_layer_bus.clone(),
        }
    }
}
