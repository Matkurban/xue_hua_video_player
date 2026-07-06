use gstreamer as gst;

/// High-level playback state reported to Dart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Idle,
    Ready,
    Buffering,
    Playing,
    Paused,
    Stopped,
    Completed,
    Error,
}

/// Discriminates which fields of [`PlayerEvent`] are meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerEventKind {
    DurationChanged,
    PositionChanged,
    VideoSize,
    StateChanged,
    Buffering,
    Eos,
    Error,
}

/// A flat event struct pushed to Dart over a broadcast stream.
#[derive(Debug, Clone)]
pub struct PlayerEvent {
    pub kind: PlayerEventKind,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub width: i32,
    pub height: i32,
    pub buffering_percent: i32,
    pub state: PlayerState,
    pub message: String,
}

impl PlayerEvent {
    pub(crate) fn base(kind: PlayerEventKind) -> Self {
        Self {
            kind,
            position_ms: 0,
            duration_ms: 0,
            width: 0,
            height: 0,
            buffering_percent: 0,
            state: PlayerState::Idle,
            message: String::new(),
        }
    }

    pub(crate) fn duration(duration_ms: i64) -> Self {
        Self {
            duration_ms,
            ..Self::base(PlayerEventKind::DurationChanged)
        }
    }

    pub(crate) fn position(position_ms: i64) -> Self {
        Self {
            position_ms,
            ..Self::base(PlayerEventKind::PositionChanged)
        }
    }

    pub(crate) fn video_size(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            ..Self::base(PlayerEventKind::VideoSize)
        }
    }

    pub(crate) fn state(state: PlayerState) -> Self {
        Self {
            state,
            ..Self::base(PlayerEventKind::StateChanged)
        }
    }

    pub(crate) fn buffering(buffering_percent: i32) -> Self {
        Self {
            buffering_percent,
            ..Self::base(PlayerEventKind::Buffering)
        }
    }

    pub(crate) fn eos() -> Self {
        Self::base(PlayerEventKind::Eos)
    }

    pub(crate) fn error(message: String) -> Self {
        Self {
            message,
            ..Self::base(PlayerEventKind::Error)
        }
    }
}

pub(crate) fn map_state(state: gst::State) -> PlayerState {
    match state {
        gst::State::Null => PlayerState::Stopped,
        gst::State::Ready => PlayerState::Ready,
        gst::State::Paused => PlayerState::Paused,
        gst::State::Playing => PlayerState::Playing,
        _ => PlayerState::Idle,
    }
}
