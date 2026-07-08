//! GStreamer 总线消息解析层（parse 阶段）。
//!
//! 将 `gst::Message` 转换为不含 GStreamer 类型的 [`BusMessage`]，供 [`super::reducer`]
//! 纯函数归约。解析阶段可安全查询 pipeline（duration、position），但不做任何状态变更。
//!
//! # English
//!
//! Parse stage: converts `gst::Message` into GStreamer-free [`BusMessage`] values for pure
//! reduction in [`super::reducer`]. May query the pipeline for duration/position; never mutates
//! state.

use gstreamer as gst;
use gstreamer::prelude::*;

use super::reducer::{BusMessage, BusPlaybackState};

/// 将单条 GStreamer 总线消息解析为 [`BusMessage`]。
///
/// # 参数
///
/// - `msg` — bus watch 收到的原始消息
/// - `pipeline` — 用于判断 `StateChanged` 来源及查询 duration/position
///
/// # 返回值
///
/// - `Some(BusMessage)` — 播放器关心的消息类型（EOS、Error、Buffering 等）
/// - `None` — 忽略的消息（如 `Element`、`Warning` 等未映射类型）
///
/// # 查询语义
///
/// | GStreamer 消息 | 额外查询 |
/// |---|---|
/// | `DurationChanged` | `query_duration` → 毫秒；失败则 `None` |
/// | `AsyncDone` | `query_position` → 可选毫秒 |
/// | `StateChanged` | 仅 pipeline 且目标态为 Paused/Playing 时查 duration |
///
/// # English
///
/// Parses one bus message into a [`BusMessage`]. Returns `None` for unhandled message types.
/// Queries duration/position where noted above; never changes pipeline state.
pub fn parse_bus_message(msg: &gst::Message, pipeline: &gst::Pipeline) -> Option<BusMessage> {
    use gst::MessageView;
    match msg.view() {
        MessageView::Eos(..) => Some(BusMessage::Eos),
        MessageView::Error(err) => Some(BusMessage::Error {
            message: format!("{} ({:?})", err.error(), err.debug()),
        }),
        MessageView::Buffering(b) => Some(BusMessage::Buffering {
            percent: b.percent(),
        }),
        MessageView::ClockLost(..) => Some(BusMessage::ClockLost),
        MessageView::DurationChanged(..) => {
            pipeline
                .query_duration::<gst::ClockTime>()
                .map(|d| BusMessage::DurationChanged {
                    duration_ms: d.mseconds() as i64,
                })
        }
        MessageView::AsyncDone(..) => {
            let position_ms = pipeline
                .query_position::<gst::ClockTime>()
                .map(|p| p.mseconds() as i64);
            Some(BusMessage::AsyncDone { position_ms })
        }
        MessageView::StateChanged(sc) => {
            let is_pipeline = sc.src().map(|s| s == pipeline).unwrap_or(false);
            let duration_ms = {
                let current = sc.current();
                if is_pipeline && (current == gst::State::Paused || current == gst::State::Playing)
                {
                    pipeline
                        .query_duration::<gst::ClockTime>()
                        .map(|d| d.mseconds() as i64)
                } else {
                    None
                }
            };
            Some(BusMessage::StateChanged {
                is_pipeline,
                old: map_playback_state(sc.old()),
                current: map_playback_state(sc.current()),
                duration_ms,
            })
        }
        MessageView::StreamCollection(..) => Some(BusMessage::StreamCollection),
        MessageView::StreamsSelected(..) => Some(BusMessage::StreamsSelected),
        _ => None,
    }
}

/// 将 `gst::State` 映射为测试友好的 [`BusPlaybackState`]。
///
/// 未知/无效 GStreamer 状态统一映射为 `Null`。
///
/// # English
///
/// Maps `gst::State` to [`BusPlaybackState`]; unknown states become `Null`.
fn map_playback_state(state: gst::State) -> BusPlaybackState {
    match state {
        gst::State::Null => BusPlaybackState::Null,
        gst::State::Ready => BusPlaybackState::Ready,
        gst::State::Paused => BusPlaybackState::Paused,
        gst::State::Playing => BusPlaybackState::Playing,
        _ => BusPlaybackState::Null,
    }
}
