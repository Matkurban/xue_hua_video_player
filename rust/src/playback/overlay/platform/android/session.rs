//! Android overlay 会话 — 附着阶段、generation 失效与绑定路径预卷 / Android overlay session — attach phase, generation invalidation, bind-path preroll.
//!
//! [`AndroidOverlaySession`] 实现 [`OverlaySession`]：JNI 回调仅缓存句柄，
//! 实际 GStreamer 绑定在 Gst 线程异步执行，并通过 generation 使陈旧工作失效。
//!
//! [`AndroidOverlaySession`] implements [`OverlaySession`]: JNI callbacks cache handles only;
//! actual GStreamer binding runs asynchronously on the Gst thread with generation-based staleness.

use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use super::ops::{
    android_pause_preroll_with_refresh, cache_android_native_window, refresh_mobile_overlay_on_gst,
};
use crate::playback::gst::clear_overlay_window_handle;
use crate::playback::overlay::gst_scheduler::{GstTaskScheduler, SpawnOnGstThreadScheduler};
use crate::playback::overlay::overlay_session::{load_preroll, OverlaySession};
use crate::playback::overlay::preroll::{
    run_bind_preroll_loop, PipelineSnapshot, PrerollEffects, PrerollResumeOutcome,
};
use crate::playback::play_resume::resume_playing;
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;

/// Android overlay 绑定阶段的单一接缝（镜像 [`super::ios_session::IosOverlaySession`]）/
/// Single seam for Android overlay bind phase (mirrors [`super::ios_session::IosOverlaySession`]).
#[derive(Clone)]
pub struct AndroidOverlaySession {
    overlay_bound: Arc<AtomicBool>,
    overlay_generation: Arc<AtomicU64>,
    attach_in_flight: Arc<AtomicBool>,
    last_width: Arc<AtomicI32>,
    last_height: Arc<AtomicI32>,
}

impl AndroidOverlaySession {
    /// 创建未绑定的 Android overlay session / Creates an unbound Android overlay session.
    pub fn new() -> Self {
        Self {
            overlay_bound: Arc::new(AtomicBool::new(false)),
            overlay_generation: Arc::new(AtomicU64::new(0)),
            attach_in_flight: Arc::new(AtomicBool::new(false)),
            last_width: Arc::new(AtomicI32::new(0)),
            last_height: Arc::new(AtomicI32::new(0)),
        }
    }

    /// 返回 overlay 工作 generation 计数器的共享引用 / Returns shared reference to the overlay work generation counter.
    pub fn overlay_generation(&self) -> Arc<AtomicU64> {
        self.overlay_generation.clone()
    }

    /// 递增 generation，使进行中的附着/清除工作失效 / Bumps generation, invalidating in-flight attach/clear work.
    pub fn bump_overlay_generation(&self) {
        self.overlay_generation.fetch_add(1, Ordering::SeqCst);
    }

    /// 是否已在 Gst 侧完成 overlay 绑定 / Whether overlay bind completed on Gst side.
    pub fn is_bound(&self) -> bool {
        self.overlay_bound.load(Ordering::SeqCst)
    }

    /// 设置 GStreamer overlay 绑定标志 / Sets the GStreamer overlay bound flag.
    pub fn set_bound(&self, bound: bool) {
        self.overlay_bound.store(bound, Ordering::SeqCst);
    }

    fn capture_generation(&self) -> u64 {
        self.overlay_generation.load(Ordering::SeqCst)
    }

    /// 返回当前工作 generation（供 JNI/调度方捕获）/ Returns current work generation (for JNI/scheduler capture).
    pub fn work_generation(&self) -> u64 {
        self.capture_generation()
    }

    fn lifecycle_stale(&self, work_generation: u64) -> bool {
        work_generation != self.capture_generation()
    }

    /// JNI 安全：仅缓存句柄/尺寸（不等待 Gst）/ JNI-safe: cache handle/dimensions only (no Gst wait).
    fn cache_surface_notify(
        &self,
        stored: &Mutex<Option<usize>>,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()> {
        let _ = (width, height);
        if handle == 0 {
            self.set_bound(false);
            cache_android_native_window(stored, 0)?;
            return Ok(());
        }
        self.set_bound(false);
        cache_android_native_window(stored, handle as usize)?;
        Ok(())
    }

    /// Surface 销毁时清除 overlay（调度 Gst 工作）/ Clears overlay on surface destroy (schedules Gst work).
    ///
    /// # 参数 / Parameters
    /// - `shell` — 管线壳层 / pipeline shell
    /// - `work_generation` — 捕获时的 generation；过期则跳过 / captured generation; skipped when stale
    /// - `scheduler` — Gst 任务调度器 / Gst task scheduler
    pub fn schedule_clear_overlay(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        work_generation: u64,
        scheduler: &dyn GstTaskScheduler,
    ) {
        let overlay_bound = self.overlay_bound.clone();
        let generation = self.overlay_generation.clone();
        scheduler.spawn(Box::new(move || {
            if work_generation != generation.load(Ordering::SeqCst) {
                return;
            }
            let guard = shell.lock();
            if let Err(e) = clear_overlay_window_handle(guard.video_sink()) {
                log::warn!("android overlay clear: {e:#}");
            }
            overlay_bound.store(false, Ordering::SeqCst);
        }));
    }

    /// Surface 绑定后异步应用 overlay（禁止在 JNI 中阻塞等待）/ Fire-and-forget apply after surface bind (never call from JNI with wait).
    pub fn schedule_apply_after_bind(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
        width: i32,
        height: i32,
        surface: VideoSurface,
        play_intent: OverlayPlayIntent,
        scheduler: &dyn GstTaskScheduler,
    ) {
        if self
            .attach_in_flight
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let session = self.clone();
        let work_generation = self.capture_generation();
        let surface = surface.clone_for_switch();
        scheduler.spawn(Box::new(move || {
            struct ApplyGuard(Arc<AtomicBool>);
            impl Drop for ApplyGuard {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::SeqCst);
                }
            }
            let _guard = ApplyGuard(session.attach_in_flight.clone());
            if session.lifecycle_stale(work_generation) {
                return;
            }
            let Some(handle) = *stored.lock() else {
                session.set_bound(false);
                return;
            };
            if let Err(e) =
                session.apply_on_gst(shell, handle, width, height, &surface, &play_intent)
            {
                session.set_bound(false);
                crate::diag::logcat_error(&format!("mobile overlay apply: {e:#}"));
            }
        }));
    }

    fn apply_on_gst(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        handle: usize,
        width: i32,
        height: i32,
        surface: &VideoSurface,
        play_intent: &OverlayPlayIntent,
    ) -> Result<()> {
        {
            let guard = shell.lock();
            refresh_mobile_overlay_on_gst(&guard, handle, width, height, "surface bind")?;
            self.set_bound(true);
            let snap = guard.snapshot();
            crate::diag::logcat_info(&format!(
                "gst: overlay applied on Gst thread — pipeline {:?} pending {:?}",
                snap.current, snap.pending
            ));
        }
        let want_play = play_intent.replay.desired_playing.load(Ordering::SeqCst);
        let mut effects = AndroidBindPrerollEffects {
            play_intent: play_intent.clone_for_async(),
            surface: surface.clone_for_switch(),
        };
        run_bind_preroll_loop(&shell, want_play, true, &mut effects)
    }
}

impl OverlaySession for AndroidOverlaySession {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
        surface_overlay_ready
    }

    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface: &VideoSurface,
        defer_log: &str,
    ) -> Result<()> {
        let gate_ready = self.gate_ready_for_load(surface.overlay_ready_for_preroll());
        load_preroll::android_apply_load_preroll(shell, gate_ready, surface, defer_log)
    }

    fn is_bound(&self) -> bool {
        self.overlay_bound.load(Ordering::SeqCst)
    }

    fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool {
        has_cached_handle && self.is_bound()
    }

    fn mark_shell_rebuilt(&self) {
        self.bump_overlay_generation();
        self.set_bound(false);
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
        shell: &PipelineShell,
        stored: Arc<Mutex<Option<usize>>>,
    ) -> Result<()> {
        if let Some(handle) = *stored.lock() {
            let (width, height) = self.cached_dimensions();
            refresh_mobile_overlay_on_gst(shell, handle, width, height, "rebind")?;
            self.set_bound(true);
            crate::diag::logcat_info("gst: overlay rebound on new video_sink");
        }
        Ok(())
    }

    fn cache_notify(
        &self,
        stored: &Arc<Mutex<Option<usize>>>,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()> {
        self.cache_surface_notify(stored, handle, width, height)
    }

    fn apply_gstreamer(
        &self,
        _shell: Arc<Mutex<PipelineShell>>,
        _stored: Arc<Mutex<Option<usize>>>,
        _surface: VideoSurface,
        _width: i32,
        _height: i32,
        _play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        Ok(())
    }

    fn notify_surface_with_shell(
        &self,
        stored: Arc<Mutex<Option<usize>>>,
        handle: i64,
        width: i32,
        height: i32,
        shell: Arc<Mutex<PipelineShell>>,
        surface: VideoSurface,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        let scheduler = default_scheduler();
        if handle == 0 {
            let work_generation = self.work_generation();
            self.cache_surface_notify(&stored, 0, 0, 0)?;
            self.schedule_clear_overlay(shell, work_generation, &scheduler);
            return Ok(());
        }
        self.cache_surface_notify(&stored, handle, width, height)?;
        let (w, h) = self.cached_dimensions();
        self.schedule_apply_after_bind(shell, stored, w, h, surface, play_intent, &scheduler);
        Ok(())
    }
}

struct AndroidBindPrerollEffects {
    play_intent: OverlayPlayIntent,
    surface: VideoSurface,
}

impl PrerollEffects for AndroidBindPrerollEffects {
    fn pause_preroll(
        &mut self,
        shell: &Arc<Mutex<PipelineShell>>,
        _snapshot: PipelineSnapshot,
    ) -> Result<()> {
        android_pause_preroll_with_refresh(
            &shell.lock(),
            &self.surface,
            Some("gst: overlay bound — starting Paused preroll"),
        )
    }

    fn resume_playing(
        &mut self,
        shell: &Arc<Mutex<PipelineShell>>,
        snapshot: PipelineSnapshot,
    ) -> Result<PrerollResumeOutcome> {
        if snapshot.pending != gst::State::VoidPending {
            crate::diag::logcat_info(&format!(
                "gst: overlay bind — pipeline pending {:?}, current {:?}",
                snapshot.pending, snapshot.current
            ));
            crate::diag::logcat_info("gst: overlay bound — resuming play while pending");
        } else {
            crate::diag::logcat_info("gst: overlay bound — resuming play (desired_playing=true)");
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

/// 返回生产环境 Gst 线程调度器 / Returns the production Gst-thread scheduler.
pub fn default_scheduler() -> SpawnOnGstThreadScheduler {
    SpawnOnGstThreadScheduler
}
