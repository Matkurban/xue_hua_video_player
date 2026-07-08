//! Overlay 门控预卷转换规则 — Ready→Paused→Playing 决策的单一来源。
//!
//! [`decide_preroll_action`] 为纯函数：给定管线快照、播放意图与 overlay 就绪状态，
//! 返回下一步应执行的转换，实际副作用由调用方或 [`super::executor`] 执行。
//!
//! Overlay-gated preroll transition rules — single source for Ready→Paused→Playing decisions.
//!
//! [`decide_preroll_action`] is pure: given a pipeline snapshot, play intent, and overlay
//! readiness, it returns the next transition; side effects run at call sites or in [`super::executor`].

use gstreamer as gst;

/// 用于纯预卷决策的管线状态快照（`decide` 内不调用实时 GStreamer API）/
/// Pipeline state snapshot for pure preroll decisions (no live GStreamer calls in `decide`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineSnapshot {
    /// 当前 GStreamer 状态 / current GStreamer state
    pub current: gst::State,
    /// 待处理目标状态 / pending target state
    pub pending: gst::State,
    /// 是否已有待加载媒体（URI 已设置）/ whether media is pending (URI set)
    pub has_pending_media: bool,
}

/// 下一步 overlay 门控管线转换（执行保留在调用点）/
/// Next overlay-gated pipeline transition (execution stays at call sites).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrerollAction {
    /// 条件未满足；不转换 / Conditions not met; no transition.
    Noop,
    /// overlay 未就绪 — 延迟预卷直至绑定（加载路径）/ Overlay not ready — defer preroll until bind (load paths).
    Defer,
    /// Ready + 待加载媒体 → Paused 预卷 / Ready + pending media → Paused preroll.
    PausePreroll,
    /// 用户意图播放且处于 Paused（含 pending≠VoidPending 恢复）/ User wants play while Paused (including pending≠VoidPending resume).
    ResumePlaying,
}

/// 纯决策：给定快照 + 意图，下一步应执行何种转换？/ Pure decision: given snapshot + intent, what transition should run next?
///
/// `overlay_ready` 为**上下文相关**标志（参见 grilling #4）：Android 绑定路径传 `true`；
/// iOS `setUri` 传 `false`；桌面 URI 加载在调用点反转句柄缓存语义。
///
/// `overlay_ready` is **context-specific** (see grilling #4): Android bind passes `true`;
/// iOS `setUri` passes `false`; desktop URI load inverts handle cache semantics at the call site.
///
/// # 参数 / Parameters
/// - `snapshot` — 管线 `current`/`pending`/媒体标志快照 / pipeline `current`/`pending`/media snapshot
/// - `want_play` — 用户是否意图播放（绑定路径可为 `true`）/ whether the user wants playback (may be `true` on bind paths)
/// - `overlay_ready` — 当前上下文下 overlay 是否就绪 / whether overlay is ready in this context
///
/// # 返回值 / Returns
/// - 下一步 [`PrerollAction`]；调用方负责执行或记录 defer / next [`PrerollAction`]; caller executes or logs defer
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
