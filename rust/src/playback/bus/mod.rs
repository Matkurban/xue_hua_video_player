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
//! 入口函数 [`attach_gst_bus_handlers`] 在 Gst MainContext 上注册 bus watch 与位置轮询定时器。
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
fn ios_overlay_bound(_ios_layer_bus: &Option<Arc<Mutex<Option<IosLayerBackend>>>>) -> bool {
    // Texture/appsink path: no CALayer attach — always ready (matches overlay_ready_for_play).
    true
}

/// 在 Gst 线程 MainContext 上安装 bus watch 与位置轮询定时器。
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
/// Installs a bus watch and a 200 ms position poll on the Gst thread's MainContext. Each
/// message follows the parse → snapshot → reduce → replay patch → emit events → apply side
/// effects pipeline documented in the module root.
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
    let rate_bus = rate.clone();
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
                rate: *rate_bus.lock(),
                #[cfg(target_os = "ios")]
                ios_layer_bus: &ios_layer_bus,
                emit: &mut emit_to_dart,
            };
            effects::apply_bus_side_effects(&reduction.effects, &mut effect_ctx);

            gst::glib::ControlFlow::Continue
        })
        .map_err(|e| anyhow!("bus watch failed: {e}"))?;

    let pipeline_pos = pipeline.clone();
    let ctx = crate::gst::gst_main_context()?.clone();
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
