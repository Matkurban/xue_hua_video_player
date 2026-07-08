//! Overlay-gated preroll transition rules — single source for Ready→Paused→Playing decisions.

use gstreamer as gst;

/// Pipeline state snapshot for pure preroll decisions (no live GStreamer calls in `decide`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineSnapshot {
    pub current: gst::State,
    pub pending: gst::State,
    pub has_pending_media: bool,
}

/// Next overlay-gated pipeline transition (execution stays at call sites).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrerollAction {
    /// Conditions not met; no transition.
    Noop,
    /// Overlay not ready — defer preroll until bind (load paths).
    Defer,
    /// Ready + pending media → Paused preroll.
    PausePreroll,
    /// User wants play while Paused (including pending≠Void resume).
    ResumePlaying,
}

/// Pure decision: given snapshot + intent, what transition should run next?
///
/// `overlay_ready` is **context-specific** (see grilling #4): Android bind passes `true`;
/// iOS `setUri` passes `false`; desktop URI load inverts handle cache semantics at the call site.
pub fn decide_preroll_action(
    snapshot: PipelineSnapshot,
    want_play: bool,
    overlay_ready: bool,
) -> PrerollAction {
    if !overlay_ready {
        return PrerollAction::Defer;
    }

    let PipelineSnapshot {
        current,
        pending,
        has_pending_media,
    } = snapshot;

    if !want_play {
        if pending == gst::State::VoidPending && current == gst::State::Ready && has_pending_media {
            return PrerollAction::PausePreroll;
        }
        return PrerollAction::Noop;
    }

    if pending != gst::State::VoidPending {
        if current == gst::State::Paused {
            return PrerollAction::ResumePlaying;
        }
        return PrerollAction::Noop;
    }

    if current == gst::State::Ready && has_pending_media {
        return PrerollAction::PausePreroll;
    }

    if current != gst::State::Ready && current != gst::State::Paused {
        return PrerollAction::Noop;
    }

    if current == gst::State::Paused {
        return PrerollAction::ResumePlaying;
    }

    PrerollAction::Noop
}

#[cfg(test)]
mod tests {
    use super::*;

    const READY_URI: PipelineSnapshot = PipelineSnapshot {
        current: gst::State::Ready,
        pending: gst::State::VoidPending,
        has_pending_media: true,
    };

    const PAUSED: PipelineSnapshot = PipelineSnapshot {
        current: gst::State::Paused,
        pending: gst::State::VoidPending,
        has_pending_media: true,
    };

    const PAUSED_PENDING_PLAY: PipelineSnapshot = PipelineSnapshot {
        current: gst::State::Paused,
        pending: gst::State::Playing,
        has_pending_media: true,
    };

    const PLAYING: PipelineSnapshot = PipelineSnapshot {
        current: gst::State::Playing,
        pending: gst::State::VoidPending,
        has_pending_media: true,
    };

    const READY_NO_MEDIA: PipelineSnapshot = PipelineSnapshot {
        current: gst::State::Ready,
        pending: gst::State::VoidPending,
        has_pending_media: false,
    };

    #[test]
    fn overlay_not_ready_defer() {
        assert_eq!(
            decide_preroll_action(READY_URI, false, false),
            PrerollAction::Defer
        );
        assert_eq!(
            decide_preroll_action(READY_URI, true, false),
            PrerollAction::Defer
        );
    }

    #[test]
    fn load_path_pause_preroll_without_want_play() {
        assert_eq!(
            decide_preroll_action(READY_URI, false, true),
            PrerollAction::PausePreroll
        );
    }

    #[test]
    fn load_path_noop_when_no_media() {
        assert_eq!(
            decide_preroll_action(READY_NO_MEDIA, false, true),
            PrerollAction::Noop
        );
    }

    #[test]
    fn bind_path_ready_then_resume_when_want_play() {
        assert_eq!(
            decide_preroll_action(READY_URI, true, true),
            PrerollAction::PausePreroll
        );
        assert_eq!(
            decide_preroll_action(PAUSED, true, true),
            PrerollAction::ResumePlaying
        );
    }

    #[test]
    fn bind_path_resume_while_pending() {
        assert_eq!(
            decide_preroll_action(PAUSED_PENDING_PLAY, true, true),
            PrerollAction::ResumePlaying
        );
    }

    #[test]
    fn bind_path_noop_when_pending_not_paused() {
        assert_eq!(
            decide_preroll_action(
                PipelineSnapshot {
                    current: gst::State::Ready,
                    pending: gst::State::Playing,
                    has_pending_media: true,
                },
                true,
                true
            ),
            PrerollAction::Noop
        );
    }

    #[test]
    fn bind_path_noop_when_playing() {
        assert_eq!(
            decide_preroll_action(PLAYING, true, true),
            PrerollAction::Noop
        );
    }

    #[test]
    fn bind_path_noop_when_not_want_play_and_already_paused() {
        assert_eq!(
            decide_preroll_action(PAUSED, false, true),
            PrerollAction::Noop
        );
    }

    #[test]
    fn bind_path_noop_when_not_want_play_pending() {
        assert_eq!(
            decide_preroll_action(PAUSED_PENDING_PLAY, false, true),
            PrerollAction::Noop
        );
    }

    #[test]
    fn table_driven_matrix() {
        let cases: &[(PipelineSnapshot, bool, bool, PrerollAction)] = &[
            (READY_URI, false, true, PrerollAction::PausePreroll),
            (READY_URI, false, false, PrerollAction::Defer),
            (READY_URI, true, true, PrerollAction::PausePreroll),
            (PAUSED, true, true, PrerollAction::ResumePlaying),
            (PAUSED, false, true, PrerollAction::Noop),
            (
                PAUSED_PENDING_PLAY,
                true,
                true,
                PrerollAction::ResumePlaying,
            ),
            (PLAYING, true, true, PrerollAction::Noop),
            (READY_NO_MEDIA, true, true, PrerollAction::Noop),
        ];
        for (snapshot, want_play, overlay_ready, expected) in cases {
            assert_eq!(
                decide_preroll_action(*snapshot, *want_play, *overlay_ready),
                *expected,
                "snapshot={snapshot:?} want_play={want_play} overlay_ready={overlay_ready}"
            );
        }
    }
}
