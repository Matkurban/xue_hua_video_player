use crate::player_events::{PlayerEvent, PlayerState};

/// Playback element state carried on bus messages (gst-free for tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusPlaybackState {
    Null,
    Ready,
    Paused,
    Playing,
}

impl BusPlaybackState {
    pub(crate) fn to_player_state(self) -> PlayerState {
        match self {
            Self::Null => PlayerState::Stopped,
            Self::Ready => PlayerState::Ready,
            Self::Paused => PlayerState::Paused,
            Self::Playing => PlayerState::Playing,
        }
    }
}

/// Read-only inputs for pure bus message reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusSnapshot {
    pub desired_playing: bool,
    pub looping: bool,
    pub is_playbin: bool,
    pub overlay_bound: bool,
}

impl BusSnapshot {
    pub fn new(
        desired_playing: bool,
        looping: bool,
        is_playbin: bool,
        overlay_bound: bool,
    ) -> Self {
        Self {
            desired_playing,
            looping,
            is_playbin,
            overlay_bound,
        }
    }
}

/// Parsed bus message (no GStreamer types).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusMessage {
    Eos,
    Error {
        message: String,
    },
    Buffering {
        percent: i32,
    },
    ClockLost,
    DurationChanged {
        duration_ms: i64,
    },
    AsyncDone {
        position_ms: Option<i64>,
    },
    StateChanged {
        is_pipeline: bool,
        old: BusPlaybackState,
        current: BusPlaybackState,
        duration_ms: Option<i64>,
    },
    StreamCollection,
    StreamsSelected,
}

/// Atomic replay flags to apply before emitting events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BusReplayPatch {
    pub at_eos: Option<bool>,
    pub desired_playing: Option<bool>,
}

/// Imperative side effects executed on the Gst thread after reduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusSideEffect {
    EosLoopSeek,
    PausePipelineForBuffering,
    ResumePipelineAfterBuffering,
    ClockLostRecover,
    #[cfg(target_os = "ios")]
    IosSetBufferingActive(bool),
    #[cfg(target_os = "ios")]
    IosSetPendingPlayAfterOverlay,
    #[cfg(target_os = "ios")]
    IosScheduleApply,
    #[cfg(target_os = "ios")]
    IosScheduleAttach,
    TrackCacheSyncFromCollection,
    TrackCacheMarkSelected,
}

#[derive(Debug, Clone)]
pub struct BusReduction {
    pub events: Vec<PlayerEvent>,
    pub effects: Vec<BusSideEffect>,
    pub replay_patch: BusReplayPatch,
}

pub fn reduce_bus_message(message: BusMessage, snapshot: BusSnapshot) -> BusReduction {
    match message {
        BusMessage::Eos => reduce_eos(snapshot),
        BusMessage::Error { message } => BusReduction {
            events: vec![
                PlayerEvent::error(message),
                PlayerEvent::state(PlayerState::Error),
            ],
            effects: Vec::new(),
            replay_patch: BusReplayPatch::default(),
        },
        BusMessage::Buffering { percent } => reduce_buffering(percent, snapshot),
        BusMessage::ClockLost => reduce_clock_lost(snapshot),
        BusMessage::DurationChanged { duration_ms } => BusReduction {
            events: vec![PlayerEvent::duration(duration_ms)],
            effects: Vec::new(),
            replay_patch: BusReplayPatch::default(),
        },
        BusMessage::AsyncDone { position_ms } => reduce_async_done(position_ms, snapshot),
        BusMessage::StateChanged {
            is_pipeline,
            old,
            current,
            duration_ms,
        } => reduce_state_changed(is_pipeline, old, current, duration_ms, snapshot),
        BusMessage::StreamCollection => reduce_stream_collection(snapshot),
        BusMessage::StreamsSelected => reduce_streams_selected(snapshot),
    }
}

fn reduce_eos(snapshot: BusSnapshot) -> BusReduction {
    if snapshot.looping {
        BusReduction {
            events: Vec::new(),
            effects: vec![BusSideEffect::EosLoopSeek],
            replay_patch: BusReplayPatch {
                at_eos: Some(false),
                desired_playing: None,
            },
        }
    } else {
        BusReduction {
            events: vec![
                PlayerEvent::eos(),
                PlayerEvent::state(PlayerState::Completed),
            ],
            effects: Vec::new(),
            replay_patch: BusReplayPatch {
                at_eos: Some(true),
                desired_playing: Some(false),
            },
        }
    }
}

fn reduce_buffering(percent: i32, snapshot: BusSnapshot) -> BusReduction {
    let mut events = vec![PlayerEvent::buffering(percent)];
    let mut effects = Vec::new();

    if !snapshot.desired_playing {
        return BusReduction {
            events,
            effects,
            replay_patch: BusReplayPatch::default(),
        };
    }

    #[cfg(target_os = "ios")]
    {
        if percent < 100 {
            events.push(PlayerEvent::state(PlayerState::Buffering));
            effects.push(BusSideEffect::IosSetBufferingActive(true));
        } else {
            effects.push(BusSideEffect::IosSetBufferingActive(false));
            if !snapshot.overlay_bound {
                effects.push(BusSideEffect::IosSetPendingPlayAfterOverlay);
            }
            if snapshot.desired_playing && snapshot.overlay_bound {
                events.push(PlayerEvent::state(PlayerState::Playing));
            }
        }
        effects.push(BusSideEffect::IosScheduleApply);
        return BusReduction {
            events,
            effects,
            replay_patch: BusReplayPatch::default(),
        };
    }

    #[cfg(not(target_os = "ios"))]
    {
        if percent < 100 {
            events.push(PlayerEvent::state(PlayerState::Buffering));
            effects.push(BusSideEffect::PausePipelineForBuffering);
        } else {
            effects.push(BusSideEffect::ResumePipelineAfterBuffering);
        }
        BusReduction {
            events,
            effects,
            replay_patch: BusReplayPatch::default(),
        }
    }
}

fn reduce_clock_lost(snapshot: BusSnapshot) -> BusReduction {
    if !snapshot.desired_playing {
        return BusReduction::default_empty();
    }

    #[cfg(target_os = "ios")]
    {
        let mut effects = Vec::new();
        if !snapshot.overlay_bound {
            effects.push(BusSideEffect::IosSetPendingPlayAfterOverlay);
        }
        effects.push(BusSideEffect::IosScheduleApply);
        return BusReduction {
            events: Vec::new(),
            effects,
            replay_patch: BusReplayPatch::default(),
        };
    }

    #[cfg(not(target_os = "ios"))]
    BusReduction {
        events: Vec::new(),
        effects: vec![BusSideEffect::ClockLostRecover],
        replay_patch: BusReplayPatch::default(),
    }
}

fn reduce_async_done(position_ms: Option<i64>, snapshot: BusSnapshot) -> BusReduction {
    #[cfg(target_os = "ios")]
    let effects = vec![BusSideEffect::IosScheduleAttach];
    #[cfg(not(target_os = "ios"))]
    let effects = Vec::new();

    let mut events = Vec::new();
    if snapshot.desired_playing {
        if let Some(position_ms) = position_ms {
            events.push(PlayerEvent::position(position_ms));
        }
    }

    BusReduction {
        events,
        effects,
        replay_patch: BusReplayPatch::default(),
    }
}

fn reduce_state_changed(
    is_pipeline: bool,
    _old: BusPlaybackState,
    current: BusPlaybackState,
    duration_ms: Option<i64>,
    snapshot: BusSnapshot,
) -> BusReduction {
    if !is_pipeline {
        return BusReduction::default_empty();
    }

    #[cfg(target_os = "ios")]
    let effects = if _old == BusPlaybackState::Ready && current == BusPlaybackState::Paused {
        vec![BusSideEffect::IosScheduleAttach]
    } else {
        Vec::new()
    };
    #[cfg(not(target_os = "ios"))]
    let effects = Vec::new();

    let mut events = Vec::new();
    if current == BusPlaybackState::Paused && snapshot.desired_playing {
        // Preroll / waiting for overlay: surface as Buffering so Dart is not stuck on Idle.
        events.push(PlayerEvent::state(PlayerState::Buffering));
    } else {
        events.push(PlayerEvent::state(current.to_player_state()));
    }
    if current == BusPlaybackState::Playing && snapshot.desired_playing {
        events.push(PlayerEvent::buffering(100));
    }
    if let Some(duration_ms) = duration_ms {
        events.push(PlayerEvent::duration(duration_ms));
    }

    BusReduction {
        events,
        effects,
        replay_patch: BusReplayPatch::default(),
    }
}

fn reduce_stream_collection(snapshot: BusSnapshot) -> BusReduction {
    if !snapshot.is_playbin {
        return BusReduction::default_empty();
    }
    BusReduction {
        events: vec![PlayerEvent::tracks_changed()],
        effects: vec![BusSideEffect::TrackCacheSyncFromCollection],
        replay_patch: BusReplayPatch::default(),
    }
}

fn reduce_streams_selected(snapshot: BusSnapshot) -> BusReduction {
    if !snapshot.is_playbin {
        return BusReduction::default_empty();
    }
    BusReduction {
        events: vec![PlayerEvent::tracks_changed()],
        effects: vec![BusSideEffect::TrackCacheMarkSelected],
        replay_patch: BusReplayPatch::default(),
    }
}

impl BusReduction {
    fn default_empty() -> Self {
        Self {
            events: Vec::new(),
            effects: Vec::new(),
            replay_patch: BusReplayPatch::default(),
        }
    }
}

impl Default for BusReduction {
    fn default() -> Self {
        Self::default_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(
        desired_playing: bool,
        looping: bool,
        is_playbin: bool,
        overlay_bound: bool,
    ) -> BusSnapshot {
        BusSnapshot::new(desired_playing, looping, is_playbin, overlay_bound)
    }

    #[test]
    fn eos_looping_seeks_and_clears_at_eos() {
        let r = reduce_bus_message(BusMessage::Eos, snap(true, true, true, true));
        assert!(r.events.is_empty());
        assert_eq!(r.effects, vec![BusSideEffect::EosLoopSeek]);
        assert_eq!(r.replay_patch.at_eos, Some(false));
    }

    #[test]
    fn eos_non_looping_completes() {
        let r = reduce_bus_message(BusMessage::Eos, snap(true, false, true, true));
        assert_eq!(r.events.len(), 2);
        assert_eq!(r.events[0].kind, crate::player_events::PlayerEventKind::Eos);
        assert_eq!(r.events[1].state, PlayerState::Completed);
        assert_eq!(r.replay_patch.at_eos, Some(true));
        assert_eq!(r.replay_patch.desired_playing, Some(false));
    }

    #[test]
    fn error_emits_error_state() {
        let r = reduce_bus_message(
            BusMessage::Error {
                message: "boom".into(),
            },
            snap(true, false, true, true),
        );
        assert_eq!(r.events.len(), 2);
        assert_eq!(r.events[0].message, "boom");
        assert_eq!(r.events[1].state, PlayerState::Error);
    }

    #[test]
    fn buffering_ignored_when_not_desired_playing() {
        let r = reduce_bus_message(
            BusMessage::Buffering { percent: 50 },
            snap(false, false, true, true),
        );
        assert_eq!(r.events.len(), 1);
        assert_eq!(r.events[0].buffering_percent, 50);
        assert!(r.effects.is_empty());
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn buffering_pauses_below_100() {
        let r = reduce_bus_message(
            BusMessage::Buffering { percent: 50 },
            snap(true, false, true, true),
        );
        assert_eq!(r.events.len(), 2);
        assert_eq!(r.events[1].state, PlayerState::Buffering);
        assert_eq!(r.effects, vec![BusSideEffect::PausePipelineForBuffering]);
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn buffering_resumes_at_100() {
        let r = reduce_bus_message(
            BusMessage::Buffering { percent: 100 },
            snap(true, false, true, true),
        );
        assert_eq!(r.events.len(), 1);
        assert_eq!(r.effects, vec![BusSideEffect::ResumePipelineAfterBuffering]);
    }

    #[cfg(target_os = "ios")]
    #[test]
    fn ios_buffering_100_emits_playing_when_overlay_ready() {
        let r = reduce_bus_message(
            BusMessage::Buffering { percent: 100 },
            snap(true, false, true, true),
        );
        assert!(r.events.iter().any(|e| e.state == PlayerState::Playing));
        assert!(r.events.iter().any(|e| e.buffering_percent == 100));
        assert!(r
            .effects
            .contains(&BusSideEffect::IosSetBufferingActive(false)));
    }

    #[cfg(target_os = "ios")]
    #[test]
    fn ios_playbin_rebuffer_cycle_recovers_to_playing() {
        let snapshot = snap(true, false, true, true);
        let _ = reduce_bus_message(BusMessage::Buffering { percent: 30 }, snapshot);
        let _ = reduce_bus_message(BusMessage::Buffering { percent: 100 }, snapshot);
        let r = reduce_bus_message(BusMessage::Buffering { percent: 20 }, snapshot);
        assert!(r.events.iter().any(|e| e.state == PlayerState::Buffering));
        let r = reduce_bus_message(BusMessage::Buffering { percent: 100 }, snapshot);
        assert!(r.events.iter().any(|e| e.state == PlayerState::Playing));
        assert!(r.events.iter().any(|e| e.buffering_percent == 100));
    }

    #[cfg(target_os = "ios")]
    #[test]
    fn ios_buffering_active_below_100() {
        let r = reduce_bus_message(
            BusMessage::Buffering { percent: 30 },
            snap(true, false, true, true),
        );
        assert!(r
            .effects
            .contains(&BusSideEffect::IosSetBufferingActive(true)));
        assert!(r.events.iter().any(|e| e.state == PlayerState::Buffering));
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn clock_lost_recovers_when_desired_playing() {
        let r = reduce_bus_message(BusMessage::ClockLost, snap(true, false, true, true));
        assert_eq!(r.effects, vec![BusSideEffect::ClockLostRecover]);
    }

    #[cfg(target_os = "ios")]
    #[test]
    fn ios_clock_lost_schedules_apply() {
        let r = reduce_bus_message(BusMessage::ClockLost, snap(true, false, true, false));
        assert!(r
            .effects
            .contains(&BusSideEffect::IosSetPendingPlayAfterOverlay));
        assert!(r.effects.contains(&BusSideEffect::IosScheduleApply));
        assert!(!r.effects.contains(&BusSideEffect::ClockLostRecover));
    }

    #[test]
    fn state_changed_maps_paused_to_buffering_when_desired_playing() {
        let r = reduce_bus_message(
            BusMessage::StateChanged {
                is_pipeline: true,
                old: BusPlaybackState::Ready,
                current: BusPlaybackState::Paused,
                duration_ms: Some(5000),
            },
            snap(true, false, true, true),
        );
        assert!(!r.events.iter().any(|e| e.state == PlayerState::Paused));
        assert!(r.events.iter().any(|e| e.state == PlayerState::Buffering));
        assert!(r.events.iter().any(|e| e.duration_ms == 5000));
    }

    #[test]
    fn state_changed_emits_playing_buffering_100() {
        let r = reduce_bus_message(
            BusMessage::StateChanged {
                is_pipeline: true,
                old: BusPlaybackState::Paused,
                current: BusPlaybackState::Playing,
                duration_ms: None,
            },
            snap(true, false, true, true),
        );
        assert!(r.events.iter().any(|e| e.state == PlayerState::Playing));
        assert!(r.events.iter().any(|e| e.buffering_percent == 100));
    }

    #[test]
    fn stream_collection_updates_tracks_for_playbin() {
        let r = reduce_bus_message(BusMessage::StreamCollection, snap(true, false, true, true));
        assert_eq!(r.events.len(), 1);
        assert_eq!(r.effects, vec![BusSideEffect::TrackCacheSyncFromCollection]);
    }

    #[test]
    fn stream_collection_ignored_for_asset() {
        let r = reduce_bus_message(BusMessage::StreamCollection, snap(true, false, false, true));
        assert!(r.events.is_empty());
        assert!(r.effects.is_empty());
    }

    #[test]
    fn async_done_emits_position_when_desired_playing() {
        let r = reduce_bus_message(
            BusMessage::AsyncDone {
                position_ms: Some(1234),
            },
            snap(true, false, true, true),
        );
        assert!(r.events.iter().any(|e| e.position_ms == 1234));
    }
}
