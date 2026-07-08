//! 总线消息纯归约层（reduce 阶段）。
//!
//! 核心入口：[`reduce_bus_message`] — 接收已解析的 [`BusMessage`] 与只读 [`BusSnapshot`]，
//! 返回 [`BusReduction`]（events + effects + replay_patch）。本模块 **不** 依赖 GStreamer，
//! 全部逻辑可单元测试。
//!
//! # 归约架构
//!
//! ```text
//! BusMessage + BusSnapshot
//!         │
//!         ▼
//! reduce_bus_message ──► BusReduction
//!         │                  ├── events: Vec<PlayerEvent>      → 发往 Dart
//!         │                  ├── effects: Vec<BusSideEffect>   → Gst 线程副作用
//!         │                  └── replay_patch: BusReplayPatch  → 原子标志（先于 events）
//!         │
//!         ├── reduce_eos
//!         ├── reduce_buffering
//!         ├── reduce_clock_lost
//!         ├── reduce_async_done
//!         ├── reduce_state_changed
//!         ├── reduce_stream_collection
//!         └── reduce_streams_selected
//! ```
//!
//! # English
//!
//! Pure reduction stage: `reduce_bus_message(BusMessage, BusSnapshot) → BusReduction`
//! containing Dart-facing events, Gst-thread side effects, and a replay patch applied before
//! events are emitted. No GStreamer types — fully unit-testable.

use crate::player_events::{PlayerEvent, PlayerState};

/// 总线消息携带的播放元素状态（无 GStreamer 依赖，便于测试）。
///
/// 通过 [`BusPlaybackState::to_player_state`] 映射为 Dart 可见的 [`PlayerState`]。
///
/// # English
///
/// Gst-free playback element state on bus messages, mapped to [`PlayerState`] for Dart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusPlaybackState {
    /// GStreamer Null → Dart Stopped
    Null,
    /// GStreamer Ready → Dart Ready
    Ready,
    /// GStreamer Paused → Dart Paused（归约中可能被改写为 Buffering）
    Paused,
    /// GStreamer Playing → Dart Playing
    Playing,
}

impl BusPlaybackState {
    /// 转换为 Dart 层 [`PlayerState`]。
    ///
    /// # English
    ///
    /// Maps to the Dart-facing [`PlayerState`].
    pub(crate) fn to_player_state(self) -> PlayerState {
        match self {
            Self::Null => PlayerState::Stopped,
            Self::Ready => PlayerState::Ready,
            Self::Paused => PlayerState::Paused,
            Self::Playing => PlayerState::Playing,
        }
    }
}

/// 纯归约的只读输入快照，取自 bus watch 时刻的原子变量与配置。
///
/// # 字段
///
/// - **`desired_playing`** — 用户是否意图播放（`desired_playing` 原子标志）
/// - **`looping`** — 是否开启循环；影响 EOS 归约（seek vs 完成）
/// - **`is_playbin`** — 当前 pipeline 是否为 playbin；非 playbin 忽略 stream 消息
/// - **`overlay_bound`** — iOS overlay 是否就绪；影响 buffering/clock-lost 的 iOS 路径
///
/// # English
///
/// Read-only snapshot for pure reduction: user intent, loop flag, playbin mode, and iOS overlay
/// readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusSnapshot {
    /// 用户意图播放 / User intends playback.
    pub desired_playing: bool,
    /// 循环模式 / Loop enabled.
    pub looping: bool,
    /// playbin pipeline（多音轨）/ playbin pipeline (multi-track).
    pub is_playbin: bool,
    /// iOS overlay 已绑定 / iOS overlay is bound and ready.
    pub overlay_bound: bool,
}

impl BusSnapshot {
    /// 构造归约快照。
    ///
    /// # English
    ///
    /// Constructs a reduction snapshot from atomic flags and configuration.
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

/// 已解析的总线消息（不含 GStreamer 类型）。
///
/// 由 [`super::parse::parse_bus_message`] 从 `gst::Message` 转换而来。
///
/// # English
///
/// Parsed bus message without GStreamer types, produced by the parse stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusMessage {
    /// 流结束 / End of stream.
    Eos,
    /// 致命错误 / Fatal pipeline error.
    Error {
        /// 格式化错误文本 / Formatted error string.
        message: String,
    },
    /// 网络缓冲进度 0–100 / Network buffering percent.
    Buffering { percent: i32 },
    /// 播放时钟丢失 / Playback clock lost.
    ClockLost,
    /// 时长变化（毫秒）/ Duration changed (milliseconds).
    DurationChanged { duration_ms: i64 },
    /// 异步 seek/state 完成 / Async operation done.
    AsyncDone {
        /// 完成时的位置（毫秒），查询失败则为 `None` / Position at completion, if known.
        position_ms: Option<i64>,
    },
    /// Pipeline 或子元素状态变化 / Element state transition.
    StateChanged {
        /// 消息是否来自顶层 pipeline / Whether the source is the top-level pipeline.
        is_pipeline: bool,
        /// 旧状态 / Previous state.
        old: BusPlaybackState,
        /// 新状态 / New state.
        current: BusPlaybackState,
        /// 可选时长（Paused/Playing 时由 parse 查询）/ Optional duration from parse stage.
        duration_ms: Option<i64>,
    },
    /// playbin 流集合更新 / playbin stream collection updated.
    StreamCollection,
    /// playbin 流选择变更 / playbin streams selected.
    StreamsSelected,
}

/// 在发送 events **之前** 写入的原子 replay 标志补丁。
///
/// `None` 字段表示不修改对应原子变量。
///
/// # English
///
/// Atomic replay-flag patch applied before emitting events. `None` fields are no-ops.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BusReplayPatch {
    /// 是否处于 EOS / Whether playback is at end-of-stream.
    pub at_eos: Option<bool>,
    /// 用户意图播放标志 / User desired-playing flag.
    pub desired_playing: Option<bool>,
}

/// 归约产出的命令式副作用，在 Gst 线程由 [`super::effects`] 执行。
///
/// # English
///
/// Imperative side effects executed on the Gst thread after reduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusSideEffect {
    /// EOS 循环：seek 到起点并保持当前 rate / Loop seek to start preserving rate.
    EosLoopSeek,
    /// 缓冲中暂停 pipeline（非 iOS）/ Pause pipeline while buffering (non-iOS).
    PausePipelineForBuffering,
    /// 缓冲完成恢复 pipeline（非 iOS）/ Resume pipeline after buffering (non-iOS).
    ResumePipelineAfterBuffering,
    /// 时钟丢失：Paused→Playing 恢复（非 iOS）/ Clock-lost recovery (non-iOS).
    ClockLostRecover,
    /// iOS：设置缓冲活跃标志 / iOS: set buffering-active on overlay backend.
    #[cfg(target_os = "ios")]
    IosSetBufferingActive(bool),
    /// iOS：overlay 就绪后补 play / iOS: defer play until overlay is ready.
    #[cfg(target_os = "ios")]
    IosSetPendingPlayAfterOverlay,
    /// iOS：调度 overlay apply / iOS: schedule overlay apply.
    #[cfg(target_os = "ios")]
    IosScheduleApply,
    /// iOS：调度 overlay attach / iOS: schedule overlay attach.
    #[cfg(target_os = "ios")]
    IosScheduleAttach,
    /// 从 StreamCollection 同步音轨缓存 / Sync track cache from stream collection.
    TrackCacheSyncFromCollection,
    /// 标记 StreamsSelected 中的选中流 / Mark selected streams in track cache.
    TrackCacheMarkSelected,
}

/// [`reduce_bus_message`] 的三路输出。
///
/// # English
///
/// Triple output of bus message reduction: Dart events, Gst side effects, replay patch.
#[derive(Debug, Clone)]
pub struct BusReduction {
    /// 发往 Dart 的播放器事件 / Events for the Dart layer.
    pub events: Vec<PlayerEvent>,
    /// Gst 线程副作用 / Side effects for the Gst thread.
    pub effects: Vec<BusSideEffect>,
    /// 先于 events 应用的原子补丁 / Atomic patch applied before events.
    pub replay_patch: BusReplayPatch,
}

/// 总线消息归约入口：将 [`BusMessage`] 与 [`BusSnapshot`] 映射为 [`BusReduction`]。
///
/// 纯函数，无副作用。各变体分派至对应的 `reduce_*` 子函数；`Error` 与 `DurationChanged`
/// 在 match 内联处理（无独立 `reduce_*`）。
///
/// # 快照输入
///
/// 所有分支均可读取完整 [`BusSnapshot`]；具体字段依赖见各 `reduce_*` 文档。
///
/// # 内联变体输出
///
/// - **`Error { message }`** — events: `error` + `Error` 状态；无 effects / replay_patch
/// - **`DurationChanged { duration_ms }`** — events: `duration`；无 effects / replay_patch
///
/// # 输出语义
///
/// - **`events`** — 立即发送给 Dart 的 [`PlayerEvent`] 列表
/// - **`effects`** — 由 [`super::effects::apply_bus_side_effects`] 在 Gst 线程执行
/// - **`replay_patch`** — 由 [`super::effects::apply_bus_replay_patch`] 在 emit 之前写入
///
/// # English
///
/// Entry point for pure bus reduction. Dispatches to per-message `reduce_*` handlers; `Error`
/// and `DurationChanged` are handled inline. Returns events, side effects, and an optional
/// replay patch with no side effects of its own.
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

/// EOS 归约。
///
/// # 快照输入
///
/// - **`snapshot.looping`** — 决定循环 seek 或正常结束
///
/// # 输出语义
///
/// | `looping` | events | effects | replay_patch |
/// |---|---|---|---|
/// | `true` | 空 | `EosLoopSeek` | `at_eos = false` |
/// | `false` | `eos` + `Completed` | 空 | `at_eos = true`, `desired_playing = false` |
///
/// # English
///
/// Reduces end-of-stream: loop seek when `snapshot.looping`, otherwise emit completion and
/// clear desired-playing.
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

/// 缓冲进度归约。
///
/// # 快照输入
///
/// - **`snapshot.desired_playing`** — `false` 时仅 emit 缓冲百分比，不触发暂停/恢复或 iOS 逻辑
/// - **`snapshot.overlay_bound`**（iOS，`percent == 100`）— 决定是否 emit `Playing` 或
///   `IosSetPendingPlayAfterOverlay`
///
/// # 输出语义
///
/// - **始终** emit `buffering(percent)`
/// - **`desired_playing == false`** — 无 effects，无 replay_patch
/// - **非 iOS，`percent < 100`** — `Buffering` 状态 + `PausePipelineForBuffering`
/// - **非 iOS，`percent == 100`** — `ResumePipelineAfterBuffering`
/// - **iOS** — `IosSetBufferingActive`、可选 `IosSetPendingPlayAfterOverlay` /
///   `Playing` 状态 + 始终 `IosScheduleApply`
///
/// # English
///
/// Reduces buffering updates. When not desired-playing, only the percent event is emitted.
/// Platform-specific pause/resume (non-iOS) or overlay scheduling (iOS) applies otherwise.
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

/// 时钟丢失归约。
///
/// # 快照输入
///
/// - **`snapshot.desired_playing`** — `false` 时返回空归约（忽略）
/// - **`snapshot.overlay_bound`**（iOS）— `false` 时追加 `IosSetPendingPlayAfterOverlay`
///
/// # 输出语义
///
/// | 平台 | `desired_playing` | events | effects |
/// |---|---|---|---|
/// | 任意 | `false` | 空 | 空 |
/// | 非 iOS | `true` | 空 | `ClockLostRecover` |
/// | iOS | `true` | 空 | `IosSetPendingPlayAfterOverlay`（若 overlay 未就绪）+ `IosScheduleApply` |
///
/// # English
///
/// Reduces clock-lost: no-op when not desired-playing; platform-specific recovery otherwise.
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

/// AsyncDone 归约（异步 seek / state 变更完成）。
///
/// # 快照输入
///
/// - **`snapshot.desired_playing`** — 仅在为 `true` 且 `position_ms` 有值时 emit position
///
/// # 输出语义
///
/// - **iOS** — 始终 `IosScheduleAttach`
/// - **events** — 条件性 `position(position_ms)`
/// - **replay_patch** — 默认（无变更）
///
/// # English
///
/// Reduces async-done: optional position event when desired-playing; iOS schedules overlay attach.
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

/// Pipeline 状态变化归约。
///
/// # 快照输入
///
/// - **`is_pipeline`** — `false` 时忽略（子元素状态变化）
/// - **`snapshot.desired_playing`** — 影响 Paused→Buffering 映射与 Playing 时的 buffering(100)
///
/// # 输出语义
///
/// - **`!is_pipeline`** — 空归约
/// - **iOS, Ready→Paused** — `IosScheduleAttach`
/// - **current == Paused && desired_playing** — emit `Buffering`（非 `Paused`，避免 Dart 卡在 Idle）
/// - **否则** — emit `current.to_player_state()`
/// - **current == Playing && desired_playing** — 追加 `buffering(100)`
/// - **`duration_ms` 有值** — 追加 `duration`
///
/// # English
///
/// Reduces pipeline state changes. Non-pipeline sources are ignored. Maps preroll Paused to
/// Buffering when the user wants playback.
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

/// StreamCollection 归约（playbin 音轨列表更新）。
///
/// # 快照输入
///
/// - **`snapshot.is_playbin`** — `false` 时返回空归约
///
/// # 输出语义
///
/// - **events** — `tracks_changed()`
/// - **effects** — `TrackCacheSyncFromCollection`
///
/// # English
///
/// Reduces stream-collection updates for playbin pipelines only.
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

/// StreamsSelected 归约（playbin 音轨选择变更）。
///
/// # 快照输入
///
/// - **`snapshot.is_playbin`** — `false` 时返回空归约
///
/// # 输出语义
///
/// - **events** — `tracks_changed()`
/// - **effects** — `TrackCacheMarkSelected`
///
/// # English
///
/// Reduces streams-selected updates for playbin pipelines only.
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
    /// 空归约：无 events、无 effects、默认 replay_patch。
    ///
    /// # English
    ///
    /// Empty reduction with no events, effects, or replay changes.
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
