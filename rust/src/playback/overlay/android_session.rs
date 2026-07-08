//! Android overlay session — attach phase, generation invalidation, bind-path preroll.

use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::playback::overlay::android::{cache_android_native_window, refresh_mobile_overlay_on_gst};
use crate::playback::overlay::gst_scheduler::{GstTaskScheduler, SpawnOnGstThreadScheduler};
use crate::playback::overlay::load_preroll::{AndroidLoadPrerollPolicy, LoadPrerollPolicy};
use crate::playback::overlay::preroll_executor::{
    run_bind_preroll_loop, PrerollEffects, PrerollResumeOutcome,
};
use crate::playback::overlay::preroll_gate::PipelineSnapshot;
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
use crate::playback::state::{resume_or_replay_from_eos, set_state_sync};
use crate::playback::surface::VideoSurface;
use crate::video::clear_overlay_window_handle;

/// Nested Android overlay state on [`crate::playback::surface::VideoSurface`].
#[cfg(target_os = "android")]
pub struct AndroidOverlayState {
    pub session: AndroidOverlaySession,
    last_width: Arc<AtomicI32>,
    last_height: Arc<AtomicI32>,
}

#[cfg(target_os = "android")]
impl AndroidOverlayState {
    pub fn new() -> Self {
        Self {
            session: AndroidOverlaySession::new(),
            last_width: Arc::new(AtomicI32::new(0)),
            last_height: Arc::new(AtomicI32::new(0)),
        }
    }

    pub fn set_cached_dimensions(&self, width: i32, height: i32) {
        if width > 0 {
            self.last_width.store(width, Ordering::SeqCst);
        }
        if height > 0 {
            self.last_height.store(height, Ordering::SeqCst);
        }
    }

    pub fn cached_dimensions(&self) -> (i32, i32) {
        (
            self.last_width.load(Ordering::SeqCst),
            self.last_height.load(Ordering::SeqCst),
        )
    }

    pub fn is_overlay_bound_on_gst(&self) -> bool {
        self.session.is_bound()
    }

    pub fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool {
        has_cached_handle && self.session.is_bound()
    }

    pub fn mark_shell_rebuilt(&self) {
        self.session.bump_overlay_generation();
        self.session.set_bound(false);
    }
}

/// Single seam for Android overlay bind phase (mirrors [`super::ios_session::IosOverlaySession`]).
#[cfg(target_os = "android")]
#[derive(Clone)]
pub struct AndroidOverlaySession {
    overlay_bound: Arc<AtomicBool>,
    overlay_generation: Arc<AtomicU64>,
    attach_in_flight: Arc<AtomicBool>,
}

#[cfg(target_os = "android")]
impl AndroidOverlaySession {
    pub fn new() -> Self {
        Self {
            overlay_bound: Arc::new(AtomicBool::new(false)),
            overlay_generation: Arc::new(AtomicU64::new(0)),
            attach_in_flight: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn overlay_generation(&self) -> Arc<AtomicU64> {
        self.overlay_generation.clone()
    }

    pub fn bump_overlay_generation(&self) {
        self.overlay_generation.fetch_add(1, Ordering::SeqCst);
    }

    pub fn is_bound(&self) -> bool {
        self.overlay_bound.load(Ordering::SeqCst)
    }

    pub fn set_bound(&self, bound: bool) {
        self.overlay_bound.store(bound, Ordering::SeqCst);
    }

    fn capture_generation(&self) -> u64 {
        self.overlay_generation.load(Ordering::SeqCst)
    }

    pub fn work_generation(&self) -> u64 {
        self.capture_generation()
    }

    fn lifecycle_stale(&self, work_generation: u64) -> bool {
        work_generation != self.capture_generation()
    }

    /// JNI-safe: cache handle/dimensions only (no Gst wait).
    pub fn cache_surface_notify(
        &self,
        stored: &Mutex<Option<usize>>,
        handle: i64,
        width: i32,
        height: i32,
        state: &AndroidOverlayState,
    ) -> Result<()> {
        if handle == 0 {
            self.set_bound(false);
            cache_android_native_window(stored, 0)?;
            return Ok(());
        }
        state.set_cached_dimensions(width, height);
        self.set_bound(false);
        cache_android_native_window(stored, handle as usize)?;
        Ok(())
    }

    /// Clears overlay on surface destroy (schedules Gst work).
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
            if let Err(e) = clear_overlay_window_handle(&guard.video_sink) {
                log::warn!("android overlay clear: {e:#}");
            }
            overlay_bound.store(false, Ordering::SeqCst);
        }));
    }

    /// Fire-and-forget apply after surface bind (never call from JNI with wait).
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
            let mut guard = shell.lock();
            let Some(handle) = *stored.lock() else {
                session.set_bound(false);
                return;
            };
            if let Err(e) =
                session.apply_on_gst(&mut guard, handle, width, height, &surface, &play_intent)
            {
                session.set_bound(false);
                crate::diag::logcat_error(&format!("mobile overlay apply: {e:#}"));
            }
        }));
    }

    fn apply_on_gst(
        &self,
        shell: &mut PipelineShell,
        handle: usize,
        width: i32,
        height: i32,
        surface: &VideoSurface,
        play_intent: &OverlayPlayIntent,
    ) -> Result<()> {
        refresh_mobile_overlay_on_gst(shell, handle, width, height, "surface bind")?;
        self.set_bound(true);
        let (_, current, pending) = shell.pipeline.state(gst::ClockTime::ZERO);
        crate::diag::logcat_info(&format!(
            "gst: overlay applied on Gst thread — pipeline {current:?} pending {pending:?}"
        ));
        let want_play = play_intent
            .replay
            .desired_playing
            .load(Ordering::SeqCst);
        let mut effects = AndroidBindPrerollEffects {
            handle,
            width,
            height,
            play_intent: play_intent.clone_for_async(),
            surface: surface.clone_for_switch(),
        };
        run_bind_preroll_loop(shell, want_play, true, &mut effects)
    }
}

#[cfg(target_os = "android")]
impl LoadPrerollPolicy for AndroidOverlaySession {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
        AndroidLoadPrerollPolicy.gate_ready_for_load(surface_overlay_ready)
    }

    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface_overlay_ready: bool,
        surface: &VideoSurface,
        defer_log: &str,
    ) -> Result<()> {
        AndroidLoadPrerollPolicy.apply_load_preroll(shell, surface_overlay_ready, surface, defer_log)
    }
}

#[cfg(target_os = "android")]
struct AndroidBindPrerollEffects {
    handle: usize,
    width: i32,
    height: i32,
    play_intent: OverlayPlayIntent,
    surface: VideoSurface,
}

#[cfg(target_os = "android")]
impl PrerollEffects for AndroidBindPrerollEffects {
    fn pause_preroll(
        &mut self,
        shell: &mut PipelineShell,
        _snapshot: PipelineSnapshot,
    ) -> Result<()> {
        crate::diag::logcat_info("gst: overlay bound — starting Paused preroll");
        set_state_sync(&shell.pipeline, gst::State::Paused)?;
        refresh_mobile_overlay_on_gst(
            shell,
            self.handle,
            self.width,
            self.height,
            "after Paused preroll",
        )
    }

    fn resume_playing(
        &mut self,
        shell: &mut PipelineShell,
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
        resume_or_replay_from_eos(
            shell,
            Some(&self.play_intent.replay),
            Some(&self.play_intent.swap),
            Some(&self.surface),
        )?;
        refresh_mobile_overlay_on_gst(
            shell,
            self.handle,
            self.width,
            self.height,
            "after Playing",
        )?;
        Ok(PrerollResumeOutcome::Finished)
    }
}

#[cfg(target_os = "android")]
pub fn default_scheduler() -> SpawnOnGstThreadScheduler {
    SpawnOnGstThreadScheduler
}
