//! iOS 总线侧 overlay 后端 — 薄适配 [`super::ios_session::IosOverlaySession`] /
//! iOS bus-facing overlay backend — thin adapter to [`super::ios_session::IosOverlaySession`].
//!
//! 在 Gst bus `READY→PAUSED` 等时机调度 CALayer 附着重试与 idle 目标状态应用。
//!
//! Schedules CALayer attach retries and idle target-state apply on Gst bus `READY→PAUSED`, etc.

use std::sync::Arc;

use gstreamer as gst;
use parking_lot::Mutex;

use super::session::{IosIdleWork, IosOverlaySession};
use crate::playback::gst_context::PlaybackGstContext;
use crate::playback::shell::PipelineShell;

/// iOS 层附着重试的共享状态（Gst 总线 `READY → PAUSED`）/
/// Shared state for iOS layer attach retries on the Gst bus (`READY → PAUSED`).
pub struct IosLayerBackend {
    ctx: Arc<PlaybackGstContext>,
    ios_session: IosOverlaySession,
    /// overlay video sink 元素槽 / overlay video sink element slot
    pub overlay_sink: Arc<Mutex<gst::Element>>,
    ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
}

impl IosLayerBackend {
    /// 从 [`PlaybackGstContext`] 构建 bus 后端 / Builds bus backend from [`PlaybackGstContext`].
    ///
    /// # 参数 / Parameters
    /// - `ctx` — 播放 Gst 上下文（含 surface 与 shell）/ playback Gst context (surface + shell)
    ///
    /// # Panics
    /// - iOS overlay sink 槽未初始化时 panic / panics when iOS overlay sink slot is unset
    pub fn from_context(ctx: Arc<PlaybackGstContext>) -> Self {
        let surface = &ctx.surface;
        Self {
            ios_session: surface.ios_session(),
            overlay_sink: surface
                .overlay_sink_slot()
                .expect("ios overlay sink slot")
                .clone(),
            ios_layer_bus_slot: surface.ios_layer_bus_slot(),
            ctx,
        }
    }

    /// 返回关联的管线壳层 / Returns the associated pipeline shell.
    ///
    /// # 返回值 / Returns
    /// - `Arc<Mutex<PipelineShell>>` 共享壳层 / shared shell
    pub fn shell(&self) -> Arc<Mutex<PipelineShell>> {
        self.ctx.shell.clone()
    }

    /// overlay 是否已绑定 / Whether the overlay is bound.
    ///
    /// # 返回值 / Returns
    /// - 委托至 [`IosOverlaySession::is_bound`] / delegates to [`IosOverlaySession::is_bound`]
    pub fn is_overlay_bound(&self) -> bool {
        self.ios_session.is_bound()
    }

    /// 标记 overlay 绑定后有待处理的播放请求 / Marks a pending play request after overlay bind.
    pub fn set_pending_play_after_overlay(&self) {
        self.ios_session.set_pending_play_after_overlay(true);
    }

    /// 设置网络缓冲活跃状态 / Sets network buffering active state.
    ///
    /// # 参数 / Parameters
    /// - `active` — 是否处于缓冲 / whether buffering is active
    pub fn set_buffering_active(&self, active: bool) {
        self.ios_session.set_buffering_active(active);
    }

    fn idle_work(&self) -> IosIdleWork {
        let play_intent = self.ctx.overlay_intent().clone_for_async();
        self.ios_session.idle_work(
            self.ctx.shell.clone(),
            self.ctx.surface.stored_handle(),
            self.ctx.surface.clone_for_switch(),
            play_intent,
            self.ios_layer_bus_slot.clone(),
        )
    }

    /// 调度 idle 目标状态应用 / Schedules idle target-state apply.
    pub fn schedule_apply(&self) {
        self.ios_session.schedule_apply(self.idle_work());
    }

    /// 调度 idle CALayer 附着重试 / Schedules idle CALayer attach retry.
    pub fn schedule_attach(&self) {
        self.ios_session.schedule_attach(self.idle_work());
    }
}
