//! iOS overlay 会话 — CALayer 附着阶段、overlay_bound 校验与 idle 目标状态门控 /
//! iOS overlay session — CALayer attach phase, verified overlay_bound, and idle target-state gate.
//!
//! [`IosOverlaySession`] 管理异步 CALayer 附着、generation 失效、缓冲暂停与绑定路径预卷循环。
//!
//! [`IosOverlaySession`] manages async CALayer attach, generation invalidation, buffering pause,
//! and bind-path preroll loops.

use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use super::bus_backend::IosLayerBackend;
use crate::gst::{gst_main_context, spawn_on_gst_thread};
use crate::playback::overlay::overlay_session::{load_preroll, OverlaySession};
use crate::playback::overlay::preroll::{
    run_bind_preroll_loop, PipelineSnapshot, PrerollEffects, PrerollResumeOutcome,
};
use crate::playback::play_resume::resume_playing;
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;

use crate::platform::ios::layer::{attach_ios_video_layer_with_completion, IosLayerAttachOutcome};

/// idle `apply_target_state` / 附着重试的工作上下文 / Work context for idle `apply_target_state` / attach retries.
pub struct IosIdleWork {
    /// 捕获时的 overlay generation / overlay generation at capture time
    pub work_generation: u64,
    /// 管线壳层 / pipeline shell
    pub shell: Arc<Mutex<PipelineShell>>,
    /// 原生宿主句柄缓存 / native host handle cache
    pub stored: Arc<Mutex<Option<usize>>>,
    /// 关联 surface / associated surface
    pub surface: VideoSurface,
    /// 播放意图 / play intent
    pub play_intent: OverlayPlayIntent,
    /// iOS bus 层后端槽 / iOS bus-layer backend slot
    pub ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
}

/// iOS CALayer 附着阶段的单一接缝：校验 `overlay_bound` 与 idle 目标状态门控 /
/// Single seam for iOS CALayer attach phase, verified overlay_bound, and idle target-state gate.
#[derive(Clone)]
pub struct IosOverlaySession {
    /// GStreamer overlay 是否已绑定 / whether GStreamer overlay is bound
    pub overlay_bound: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    overlay_generation: Arc<AtomicU64>,
    attach_in_flight: Arc<AtomicBool>,
    pending_play_after_overlay: Arc<AtomicBool>,
    buffering_active: Arc<AtomicBool>,
    apply_scheduled: Arc<AtomicBool>,
    attach_scheduled: Arc<AtomicBool>,
    state_apply_in_flight: Arc<AtomicBool>,
    /// 上次成功附着的宿主句柄 / last successfully attached host handle
    pub last_applied_handle: Arc<AtomicUsize>,
    last_width: Arc<AtomicI32>,
    last_height: Arc<AtomicI32>,
    /// iOS bus 层后端注册槽 / iOS bus-layer backend registration slot
    pub ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
    overlay_sink: Option<Arc<Mutex<gst::Element>>>,
}

impl IosOverlaySession {
    /// 使用外部共享原子状态构造 session / Constructs session with externally shared atomic state.
    ///
    /// # 参数 / Parameters
    /// - `overlay_bound` — 绑定标志 / bound flag
    /// - `last_applied_handle` — 上次附着句柄 / last attached handle
    /// - `running` — replay 存活标志 / replay lifetime flag
    /// - `overlay_generation` — 工作 generation / work generation
    pub fn new(
        overlay_bound: Arc<AtomicBool>,
        last_applied_handle: Arc<AtomicUsize>,
        running: Arc<AtomicBool>,
        overlay_generation: Arc<AtomicU64>,
    ) -> Self {
        Self {
            overlay_bound,
            running,
            overlay_generation,
            attach_in_flight: Arc::new(AtomicBool::new(false)),
            pending_play_after_overlay: Arc::new(AtomicBool::new(false)),
            buffering_active: Arc::new(AtomicBool::new(false)),
            apply_scheduled: Arc::new(AtomicBool::new(false)),
            attach_scheduled: Arc::new(AtomicBool::new(false)),
            state_apply_in_flight: Arc::new(AtomicBool::new(false)),
            last_applied_handle,
            last_width: Arc::new(AtomicI32::new(0)),
            last_height: Arc::new(AtomicI32::new(0)),
            ios_layer_bus_slot: Arc::new(Mutex::new(None)),
            overlay_sink: None,
        }
    }

    /// 使用默认原子状态与给定 `running` 标志构造 / Constructs with default atomics and given `running` flag.
    pub fn new_with_running(running: Arc<AtomicBool>) -> Self {
        Self::new(
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicUsize::new(0)),
            running,
            Arc::new(AtomicU64::new(0)),
        )
    }

    /// 接入 replay 运行标志（管线存活期）/ Wires replay running flag (pipeline lifetime).
    pub fn wire_running(&mut self, running: Arc<AtomicBool>) {
        self.running = running;
    }

    /// 设置或更新 overlay video sink 元素槽 / Sets or updates the overlay video sink element slot.
    pub fn set_overlay_sink(&mut self, element: gst::Element) {
        match &self.overlay_sink {
            Some(slot) => *slot.lock() = element,
            None => self.overlay_sink = Some(Arc::new(Mutex::new(element))),
        }
    }

    /// 返回 overlay sink 共享槽 / Returns the overlay sink shared slot.
    pub fn overlay_sink_slot(&self) -> Option<&Arc<Mutex<gst::Element>>> {
        self.overlay_sink.as_ref()
    }

    /// 注册 iOS Gst bus 层后端 / Registers the iOS Gst bus-layer backend.
    pub fn register_ios_layer_backend(&self, backend: IosLayerBackend) {
        *self.ios_layer_bus_slot.lock() = Some(backend);
    }

    /// 返回 overlay generation 共享计数器 / Returns shared overlay generation counter.
    pub fn overlay_generation(&self) -> Arc<AtomicU64> {
        self.overlay_generation.clone()
    }

    /// 递增 generation，使陈旧附着/应用工作失效 / Bumps generation, invalidating stale attach/apply work.
    pub fn bump_overlay_generation(&self) {
        self.overlay_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn capture_generation(&self) -> u64 {
        self.overlay_generation.load(Ordering::SeqCst)
    }

    fn lifecycle_stale(&self, work_generation: u64) -> bool {
        !self.running.load(Ordering::SeqCst)
            || work_generation != self.overlay_generation.load(Ordering::SeqCst)
    }

    /// 是否已在 Gst 侧完成 overlay 绑定 / Whether overlay is bound on Gst side.
    pub fn is_bound(&self) -> bool {
        self.overlay_bound.load(Ordering::SeqCst)
    }

    /// 设置「overlay 绑定后待播放」标志 / Sets the pending-play-after-overlay flag.
    pub fn set_pending_play_after_overlay(&self, pending: bool) {
        self.pending_play_after_overlay
            .store(pending, Ordering::SeqCst);
    }

    /// 设置网络缓冲活跃标志（缓冲时暂停播放）/ Sets network buffering active flag (pauses during buffering).
    pub fn set_buffering_active(&self, active: bool) {
        self.buffering_active.store(active, Ordering::SeqCst);
    }

    /// 壳层重建时重置全部附着/调度状态 / Resets all attach/schedule state on shell rebuild.
    pub fn reset_for_shell_rebuild(&self) {
        self.overlay_bound.store(false, Ordering::SeqCst);
        self.attach_in_flight.store(false, Ordering::SeqCst);
        self.state_apply_in_flight.store(false, Ordering::SeqCst);
        self.pending_play_after_overlay
            .store(false, Ordering::SeqCst);
        self.buffering_active.store(false, Ordering::SeqCst);
        self.apply_scheduled.store(false, Ordering::SeqCst);
        self.attach_scheduled.store(false, Ordering::SeqCst);
        self.last_applied_handle.store(0, Ordering::SeqCst);
    }

    /// 每次媒体变更时清除 overlay 状态（URI 重载、资源切换）/ Clears overlay state on every media change (URI reload, asset swap).
    pub fn reset_for_media_change(&self) {
        self.reset_for_shell_rebuild();
    }

    /// 宿主视图变更时部分重置（附着进行中则跳过）/ Partial reset on host view change (skipped when attach in flight).
    pub fn reset_for_host_change(&self) {
        if self.attach_in_flight.load(Ordering::SeqCst) {
            log::debug!("ios reset_for_host_change skipped (attach in flight)");
            return;
        }
        self.overlay_bound.store(false, Ordering::SeqCst);
        self.last_applied_handle.store(0, Ordering::SeqCst);
    }

    fn cache_ios_overlay(
        &self,
        stored: &Arc<Mutex<Option<usize>>>,
        handle: i64,
        width: i32,
        height: i32,
    ) {
        if handle == 0 {
            self.reset_for_host_change();
            *stored.lock() = None;
            return;
        }
        let new_handle = handle as usize;
        let host_changed = match *stored.lock() {
            Some(h) if h != 0 => h != new_handle,
            _ => false,
        };
        self.set_cached_dimensions(width, height);
        if host_changed {
            self.reset_for_host_change();
        }
        *stored.lock() = Some(new_handle);
    }

    /// 调度 idle 附着重试（bus `READY→PAUSED` / `AsyncDone`）/ Schedules idle attach retry (bus `READY→PAUSED` / `AsyncDone`).
    pub fn schedule_attach(&self, work: IosIdleWork) {
        if self.lifecycle_stale(work.work_generation) {
            return;
        }
        if self
            .attach_scheduled
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let session = self.clone();
        let work_generation = work.work_generation;
        let ctx = match gst_main_context() {
            Ok(c) => c.clone(),
            Err(e) => {
                log::warn!("ios schedule_attach: {e:#}");
                session.attach_scheduled.store(false, Ordering::SeqCst);
                return;
            }
        };
        ctx.invoke(move || {
            session.attach_scheduled.store(false, Ordering::SeqCst);
            if session.lifecycle_stale(work_generation) {
                return;
            }
            let _ = session.request_attach(
                work.shell.clone(),
                work.stored.clone(),
                work.surface.clone_for_switch(),
                work.play_intent.clone_for_async(),
                "bus attach",
                work_generation,
                work.ios_layer_bus_slot.clone(),
            );
            session.schedule_apply(work);
        });
    }

    /// 调度 idle 目标状态应用（缓冲、时钟丢失、播放恢复）/ Schedules idle target-state apply (buffering, clock-lost, play resume).
    pub fn schedule_apply(&self, work: IosIdleWork) {
        if self.lifecycle_stale(work.work_generation) {
            return;
        }
        if self
            .apply_scheduled
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let session = self.clone();
        let work_generation = work.work_generation;
        let ctx = match gst_main_context() {
            Ok(c) => c.clone(),
            Err(e) => {
                log::warn!("ios schedule_apply: {e:#}");
                session.apply_scheduled.store(false, Ordering::SeqCst);
                return;
            }
        };
        ctx.invoke(move || {
            session.apply_scheduled.store(false, Ordering::SeqCst);
            if session.lifecycle_stale(work_generation) {
                return;
            }
            if let Err(e) = session.apply_target_state(work) {
                log::warn!("ios apply_target_state: {e:#}");
            }
        });
    }

    /// 至多调度一次异步 CALayer 附着；合并并发调用 / Schedules at most one async CALayer attach; coalesces concurrent callers.
    ///
    /// # 返回值 / Returns
    /// - [`IosLayerAttachOutcome`] 结果 / [`IosLayerAttachOutcome`] result
    pub fn request_attach(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
        surface: VideoSurface,
        play_intent: OverlayPlayIntent,
        log_reason: &'static str,
        work_generation: u64,
        ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
    ) -> Result<IosLayerAttachOutcome> {
        if self.lifecycle_stale(work_generation) {
            return Ok(IosLayerAttachOutcome::Skipped);
        }

        if self.is_bound() {
            self.schedule_apply(idle_work_from_parts(
                shell,
                stored,
                surface,
                play_intent,
                work_generation,
                ios_layer_bus_slot,
            ));
            return Ok(IosLayerAttachOutcome::Skipped);
        }

        if self
            .attach_in_flight
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(IosLayerAttachOutcome::Skipped);
        }

        let host_view = match *stored.lock() {
            Some(h) if h != 0 => h,
            _ => {
                self.attach_in_flight.store(false, Ordering::SeqCst);
                return Ok(IosLayerAttachOutcome::Skipped);
            }
        };

        let session = self.clone();
        let shell_finish = shell.clone();
        let stored_finish = stored.clone();
        let surface_finish = surface.clone_for_switch();
        let attach_generation = work_generation;
        let ios_slot_finish = ios_layer_bus_slot.clone();

        let (pipeline, sink, has_pending_media) = {
            let guard = shell.lock();
            (
                guard.clone_pipeline(),
                guard.clone_video_sink(),
                guard.has_pending_media(),
            )
        };

        if self.lifecycle_stale(attach_generation) {
            self.attach_in_flight.store(false, Ordering::SeqCst);
            return Ok(IosLayerAttachOutcome::Skipped);
        }

        match attach_ios_video_layer_with_completion(
            &pipeline,
            has_pending_media,
            &sink,
            host_view,
            move |attached| {
                if !attached {
                    session.attach_in_flight.store(false, Ordering::SeqCst);
                    return;
                }
                if session.lifecycle_stale(attach_generation) {
                    session.attach_in_flight.store(false, Ordering::SeqCst);
                    return;
                }
                session.finish_attach(
                    shell_finish,
                    host_view,
                    stored_finish,
                    surface_finish,
                    play_intent,
                    log_reason,
                    attach_generation,
                    ios_slot_finish,
                );
            },
        ) {
            Ok(IosLayerAttachOutcome::LayerNotReady) => {
                self.attach_in_flight.store(false, Ordering::SeqCst);
                Ok(IosLayerAttachOutcome::LayerNotReady)
            }
            Ok(outcome @ IosLayerAttachOutcome::Scheduled) => Ok(outcome),
            Ok(outcome @ IosLayerAttachOutcome::Skipped) => {
                self.attach_in_flight.store(false, Ordering::SeqCst);
                Ok(outcome)
            }
            Err(e) => {
                self.attach_in_flight.store(false, Ordering::SeqCst);
                Err(e)
            }
        }
    }

    fn finish_attach(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        host_view: usize,
        stored: Arc<Mutex<Option<usize>>>,
        surface: VideoSurface,
        play_intent: OverlayPlayIntent,
        log_reason: &'static str,
        work_generation: u64,
        ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
    ) {
        if self.lifecycle_stale(work_generation) {
            self.attach_in_flight.store(false, Ordering::SeqCst);
            return;
        }
        self.overlay_bound.store(true, Ordering::SeqCst);
        self.attach_in_flight.store(false, Ordering::SeqCst);
        self.last_applied_handle.store(host_view, Ordering::SeqCst);
        log::info!("gst: ios layer verified attached ({log_reason})");
        self.schedule_apply(idle_work_from_parts(
            shell,
            stored,
            surface,
            play_intent,
            work_generation,
            ios_layer_bus_slot,
        ));
    }

    /// Tutorial 4 `target_state` + Tutorial 12 buffering — runs on Gst idle, never from bus stack.
    fn apply_target_state(&self, work: IosIdleWork) -> Result<()> {
        if self.lifecycle_stale(work.work_generation) {
            return Ok(());
        }

        if self
            .state_apply_in_flight
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }

        struct StateApplyGuard(Arc<AtomicBool>);
        impl Drop for StateApplyGuard {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _guard = StateApplyGuard(self.state_apply_in_flight.clone());

        if !self.is_bound() {
            let _ = self.request_attach(
                work.shell.clone(),
                work.stored.clone(),
                work.surface.clone_for_switch(),
                work.play_intent.clone_for_async(),
                "idle attach",
                work.work_generation,
                work.ios_layer_bus_slot.clone(),
            );
            if !self.is_bound() || self.lifecycle_stale(work.work_generation) {
                return Ok(());
            }
        }

        let want_play = work
            .play_intent
            .replay
            .desired_playing
            .load(Ordering::SeqCst)
            || self
                .pending_play_after_overlay
                .swap(false, Ordering::SeqCst);

        if self.lifecycle_stale(work.work_generation) {
            return Ok(());
        }

        let current = {
            let guard = work.shell.lock();
            guard.snapshot().current
        };

        if self.buffering_active.load(Ordering::SeqCst) && want_play {
            if current == gst::State::Playing {
                log::info!("gst: buffering — pausing pipeline");
                work.shell.lock().set_state_sync(gst::State::Paused)?;
            }
            return Ok(());
        }

        if !want_play {
            return Ok(());
        }

        let overlay_ready = self.is_bound();

        let mut effects = IosBindPrerollEffects {
            play_intent: work.play_intent.clone_for_async(),
            surface: work.surface.clone_for_switch(),
        };
        run_bind_preroll_loop(&work.shell, want_play, overlay_ready, &mut effects)?;

        log_ios_layer_status(&work.shell, "after resume");

        Ok(())
    }

    /// overlay 绑定后合并恢复播放 — 经 idle `apply_target_state` 路由 / Coalesced resume after overlay binds — routes through idle `apply_target_state`.
    pub fn drain_pending_play(&self, work: IosIdleWork) {
        self.schedule_apply(work);
    }

    /// 构建当前 generation 的 idle 工作上下文 / Builds idle work context for the current generation.
    pub fn idle_work(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
        surface: VideoSurface,
        play_intent: OverlayPlayIntent,
        ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
    ) -> IosIdleWork {
        idle_work_from_parts(
            shell,
            stored,
            surface,
            play_intent,
            self.capture_generation(),
            ios_layer_bus_slot,
        )
    }

    fn apply_gstreamer_inner(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
        surface: VideoSurface,
        width: i32,
        height: i32,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let (prev_w, prev_h) = self.cached_dimensions();
        self.set_cached_dimensions(width, height);
        let host_view = *stored.lock();
        let Some(host_view) = host_view else {
            return Ok(());
        };

        let last_applied = self.last_applied_handle.load(Ordering::SeqCst);
        if last_applied != 0 && last_applied != host_view {
            self.reset_for_host_change();
        }

        if self.is_bound() && self.last_applied_handle.load(Ordering::SeqCst) == host_view {
            let dimensions_changed = prev_w != width || prev_h != height;
            if dimensions_changed || play_intent.replay.desired_playing.load(Ordering::SeqCst) {
                let session = self.clone();
                let stored_clone = stored.clone();
                let surface_for_work = surface.clone_for_switch();
                let running = play_intent.replay.running.clone();
                let work_generation = session.overlay_generation().load(Ordering::SeqCst);
                let ios_intent = play_intent.clone_for_async();
                let ios_slot = self.ios_layer_bus_slot.clone();
                spawn_on_gst_thread(move || {
                    if !running.load(Ordering::SeqCst)
                        || work_generation != session.overlay_generation().load(Ordering::SeqCst)
                    {
                        return;
                    }
                    if dimensions_changed {
                        let sink = {
                            let guard = shell.lock();
                            guard.clone_video_sink()
                        };
                        if let Ok(layer) = crate::platform::ios::layer::read_sink_layer(&sink) {
                            if !crate::platform::ios::attach_layer_on_main_thread_sync(
                                host_view, layer,
                            ) {
                                crate::platform::ios::layer::release_sink_layer(layer);
                            }
                        }
                    }
                    if !running.load(Ordering::SeqCst)
                        || work_generation != session.overlay_generation().load(Ordering::SeqCst)
                    {
                        return;
                    }
                    session.drain_pending_play(session.idle_work(
                        shell,
                        stored_clone,
                        surface_for_work,
                        ios_intent,
                        ios_slot,
                    ));
                });
            }
            return Ok(());
        }

        let session = self.clone();
        let surface_for_attach = surface.clone_for_switch();
        let running = play_intent.replay.running.clone();
        let work_generation = session.overlay_generation().load(Ordering::SeqCst);
        let ios_intent = play_intent.clone_for_async();
        let ios_slot = self.ios_layer_bus_slot.clone();
        spawn_on_gst_thread(move || {
            if !running.load(Ordering::SeqCst)
                || work_generation != session.overlay_generation().load(Ordering::SeqCst)
            {
                return;
            }
            let _ = session.request_attach(
                shell,
                stored,
                surface_for_attach,
                ios_intent,
                "Swift apply",
                work_generation,
                ios_slot,
            );
        });
        Ok(())
    }
}

impl OverlaySession for IosOverlaySession {
    fn gate_ready_for_load(&self, _surface_overlay_ready: bool) -> bool {
        false
    }

    fn apply_load_preroll(
        &self,
        _shell: &PipelineShell,
        surface: &VideoSurface,
        defer_log: &str,
    ) -> Result<()> {
        let gate_ready = self.gate_ready_for_load(surface.overlay_ready_for_preroll());
        load_preroll::ios_apply_load_preroll(gate_ready, defer_log)
    }

    fn is_bound(&self) -> bool {
        self.overlay_bound.load(Ordering::SeqCst)
    }

    fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool {
        has_cached_handle
    }

    fn mark_shell_rebuilt(&self) {
        self.bump_overlay_generation();
        self.reset_for_shell_rebuild();
    }

    fn set_cached_dimensions(&self, width: i32, height: i32) {
        if width > 0 {
            self.last_width.store(width, Ordering::SeqCst);
        }
        if height > 0 {
            self.last_height.store(height, Ordering::SeqCst);
        }
    }

    fn cached_dimensions(&self) -> (i32, i32) {
        (
            self.last_width.load(Ordering::SeqCst),
            self.last_height.load(Ordering::SeqCst),
        )
    }

    fn rebind_cached_overlay(
        &self,
        _shell: &PipelineShell,
        _stored: Arc<Mutex<Option<usize>>>,
    ) -> Result<()> {
        Ok(())
    }

    fn cache_notify(
        &self,
        stored: &Arc<Mutex<Option<usize>>>,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()> {
        self.cache_ios_overlay(stored, handle, width, height);
        Ok(())
    }

    fn apply_gstreamer(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
        surface: VideoSurface,
        width: i32,
        height: i32,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        self.apply_gstreamer_inner(shell, stored, surface, width, height, play_intent)
    }
}

struct IosBindPrerollEffects {
    play_intent: OverlayPlayIntent,
    surface: VideoSurface,
}

impl PrerollEffects for IosBindPrerollEffects {
    fn pause_preroll(
        &mut self,
        shell: &Arc<Mutex<PipelineShell>>,
        _snapshot: PipelineSnapshot,
    ) -> Result<()> {
        log::info!("gst: overlay bound — starting Paused preroll");
        shell.lock().set_state_sync(gst::State::Paused)
    }

    fn resume_playing(
        &mut self,
        shell: &Arc<Mutex<PipelineShell>>,
        snapshot: PipelineSnapshot,
    ) -> Result<PrerollResumeOutcome> {
        if snapshot.pending != gst::State::VoidPending {
            log::info!(
                "gst: overlay bind — pipeline pending {:?}, current {:?}",
                snapshot.pending,
                snapshot.current
            );
            log::info!("gst: overlay bound — resuming play while pending");
        } else {
            log::info!("gst: overlay bound — resuming play (desired_playing=true)");
        }
        resume_playing(
            shell.clone(),
            &self.play_intent.replay,
            &self.play_intent.swap,
            &self.surface,
            true,
        )?;
        Ok(PrerollResumeOutcome::Finished)
    }
}

/// Diagnostic: logs the sink `AVSampleBufferDisplayLayer` status/error so device
/// runs reveal whether it is Rendering (1) or Failed (2, e.g. non-IOSurface
/// buffers). The read layer is released on the main thread.
fn log_ios_layer_status(shell: &Arc<Mutex<PipelineShell>>, context: &str) {
    let sink = {
        let guard = shell.lock();
        guard.clone_video_sink()
    };
    if let Ok(layer) = crate::platform::ios::layer::read_sink_layer(&sink) {
        let (status, error_code) = crate::platform::ios::layer_status(layer);
        log::info!("gst: ios display layer status={status} error={error_code} ({context})");
        crate::platform::ios::layer::release_sink_layer(layer);
    }
}

fn idle_work_from_parts(
    shell: Arc<Mutex<PipelineShell>>,
    stored: Arc<Mutex<Option<usize>>>,
    surface: VideoSurface,
    play_intent: OverlayPlayIntent,
    work_generation: u64,
    ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
) -> IosIdleWork {
    IosIdleWork {
        work_generation,
        shell,
        stored,
        surface,
        play_intent,
        ios_layer_bus_slot,
    }
}
