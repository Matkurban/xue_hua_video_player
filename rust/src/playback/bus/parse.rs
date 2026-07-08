use gstreamer as gst;
use gstreamer::prelude::*;

use super::reducer::{BusMessage, BusPlaybackState};

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

fn map_playback_state(state: gst::State) -> BusPlaybackState {
    match state {
        gst::State::Null => BusPlaybackState::Null,
        gst::State::Ready => BusPlaybackState::Ready,
        gst::State::Paused => BusPlaybackState::Paused,
        gst::State::Playing => BusPlaybackState::Playing,
        _ => BusPlaybackState::Null,
    }
}
