use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::gst_runtime::gst_main_context;
use crate::playback::overlay::preroll_gate::{
    decide_preroll_action, PipelineSnapshot, PrerollAction,
};
use crate::playback::overlay::IosLayerBackend;
use crate::playback::replay::{replay_asset_shell, OverlayPlayIntent};
use crate::playback::shell::{PipelineShell, SourceKind};
use crate::playback::state::set_state_sync;

use crate::video::ios_layer::{attach_ios_video_layer_with_completion, IosLayerAttachOutcome};

/// Work context for idle `apply_target_state` / attach retries.
#[derive(Clone)]
pub struct IosIdleWork {
    pub work_generation: u64,
    pub shell: Arc<Mutex<PipelineShell>>,
    pub stored: Arc<Mutex<Option<usize>>>,
    pub play_intent: OverlayPlayIntent,
    pub ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
}

/// Single seam for iOS CALayer attach phase, verified overlay_bound, and idle target-state gate.
#[derive(Clone)]
pub struct IosOverlaySession {
    pub overlay_bound: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    overlay_generation: Arc<AtomicU64>,
    attach_in_flight: Arc<AtomicBool>,
    pending_play_after_overlay: Arc<AtomicBool>,
    buffering_active: Arc<AtomicBool>,
    apply_scheduled: Arc<AtomicBool>,
    attach_scheduled: Arc<AtomicBool>,
    state_apply_in_flight: Arc<AtomicBool>,
    pub last_applied_handle: Arc<AtomicUsize>,
}

impl IosOverlaySession {
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
        }
    }

    pub fn overlay_generation(&self) -> Arc<AtomicU64> {
        self.overlay_generation.clone()
    }

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

    pub fn is_bound(&self) -> bool {
        self.overlay_bound.load(Ordering::SeqCst)
    }

    pub fn set_pending_play_after_overlay(&self, pending: bool) {
        self.pending_play_after_overlay
            .store(pending, Ordering::SeqCst);
    }

    pub fn set_buffering_active(&self, active: bool) {
        self.buffering_active.store(active, Ordering::SeqCst);
    }

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

    /// Clears overlay state on every media change (URI reload, asset swap).
    pub fn reset_for_media_change(&self) {
        self.reset_for_shell_rebuild();
    }

    pub fn reset_for_host_change(&self) {
        if self.attach_in_flight.load(Ordering::SeqCst) {
            log::debug!("ios reset_for_host_change skipped (attach in flight)");
            return;
        }
        self.overlay_bound.store(false, Ordering::SeqCst);
        self.last_applied_handle.store(0, Ordering::SeqCst);
    }

    /// Schedules idle attach retry (bus `READY→PAUSED` / `AsyncDone`).
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
                work.play_intent.clone_for_async(),
                "bus attach",
                work_generation,
            );
            session.schedule_apply(work);
        });
    }

    /// Schedules idle target-state apply (buffering, clock-lost, play resume).
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

    /// Schedules at most one async CALayer attach; coalesces concurrent callers.
    pub fn request_attach(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
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
        let attach_generation = work_generation;
        let ios_slot_finish = ios_layer_bus_slot.clone();

        let (pipeline, sink, has_pending_media) = {
            let guard = shell.lock();
            (
                guard.pipeline.clone(),
                guard.video_sink.clone(),
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
            play_intent.bundle.shell.surface.stored_handle(),
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
            .bundle
            .replay
            .desired_playing
            .load(Ordering::SeqCst)
            || self
                .pending_play_after_overlay
                .swap(false, Ordering::SeqCst);

        let snapshot = {
            let guard = work.shell.lock();
            let (_, current, pending) = guard.pipeline.state(gst::ClockTime::ZERO);
            (guard.pipeline.clone(), guard.kind, current, pending)
        };

        if self.lifecycle_stale(work.work_generation) {
            return Ok(());
        }

        let (pipeline, kind, current, _pending) = snapshot;

        if self.buffering_active.load(Ordering::SeqCst) && want_play {
            if current == gst::State::Playing {
                log::info!("gst: buffering — pausing pipeline");
                set_state_sync(&pipeline, gst::State::Paused)?;
            }
            return Ok(());
        }

        if !want_play {
            return Ok(());
        }

        let overlay_ready = self.is_bound();

        for _ in 0..4 {
            let snapshot = PipelineSnapshot::from_shell(&work.shell.lock());
            let action = decide_preroll_action(snapshot, want_play, overlay_ready);
            match action {
                PrerollAction::Noop | PrerollAction::Defer => break,
                PrerollAction::PausePreroll => {
                    log::info!("gst: overlay bound — starting Paused preroll");
                    set_state_sync(&pipeline, gst::State::Paused)?;
                }
                PrerollAction::ResumePlaying => {
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
                    return resume_pipeline_playing(
                        work.shell,
                        &work.play_intent,
                        &pipeline,
                        kind,
                        &work.ios_layer_bus_slot,
                    );
                }
            }
        }

        Ok(())
    }

    /// Coalesced resume after overlay binds — routes through idle `apply_target_state`.
    pub fn drain_pending_play(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: &OverlayPlayIntent,
        work_generation: u64,
        ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
    ) {
        self.schedule_apply(idle_work_from_parts(
            shell,
            play_intent.bundle.shell.surface.stored_handle(),
            play_intent.clone_for_async(),
            work_generation,
            ios_layer_bus_slot,
        ));
    }

    pub fn idle_work(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
        play_intent: OverlayPlayIntent,
        ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
    ) -> IosIdleWork {
        idle_work_from_parts(
            shell,
            stored,
            play_intent,
            self.capture_generation(),
            ios_layer_bus_slot,
        )
    }
}

fn idle_work_from_parts(
    shell: Arc<Mutex<PipelineShell>>,
    stored: Arc<Mutex<Option<usize>>>,
    play_intent: OverlayPlayIntent,
    work_generation: u64,
    ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
) -> IosIdleWork {
    IosIdleWork {
        work_generation,
        shell,
        stored,
        play_intent,
        ios_layer_bus_slot,
    }
}

/// Resumes PLAYING without holding `shell` across `set_state_sync` (bus may re-enter attach).
fn resume_pipeline_playing(
    shell: Arc<Mutex<PipelineShell>>,
    play_intent: &OverlayPlayIntent,
    pipeline: &gst::Pipeline,
    kind: SourceKind,
    ios_layer_bus_slot: &Arc<Mutex<Option<IosLayerBackend>>>,
) -> Result<()> {
    if play_intent.bundle.replay.at_eos.load(Ordering::SeqCst) {
        play_intent
            .bundle
            .replay
            .at_eos
            .store(false, Ordering::SeqCst);
        match kind {
            SourceKind::Uri => {
                pipeline
                    .seek_simple(
                        gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                        gst::ClockTime::ZERO,
                    )
                    .map_err(|e| anyhow::anyhow!("seek to start before play: {e}"))?;
            }
            SourceKind::Asset => {
                let mut guard = shell.lock();
                return replay_asset_shell(
                    &mut guard,
                    &play_intent.bundle,
                    Some(ios_layer_bus_slot),
                );
            }
        }
    }
    set_state_sync(pipeline, gst::State::Playing)
}
