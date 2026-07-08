//! Unified play / EOS resume — single interface for all platforms.

use std::sync::{
    atomic::Ordering,
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::replay::{replay_asset_shell, PlayReplayContext};
use crate::playback::shell::{PipelineShell, SourceKind};
use crate::playback::surface::VideoSurface;
use crate::playback::switch::PipelineSwapConfig;

#[cfg(target_os = "android")]
use crate::playback::overlay::refresh_mobile_overlay_on_gst;

/// Planned pipeline action for play/EOS resume (pure decision — test surface).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResumeAction {
    DeferOverlay,
    SetPlaying,
    SeekToStartAndPlay,
    ReplayAssetShell,
}

/// Maps overlay readiness + EOS state + shell kind to the resume action.
pub(crate) fn plan_resume_action(
    overlay_ready: bool,
    at_eos: bool,
    kind: SourceKind,
) -> ResumeAction {
    if !overlay_ready {
        return ResumeAction::DeferOverlay;
    }
    if !at_eos {
        return ResumeAction::SetPlaying;
    }
    match kind {
        SourceKind::Uri => ResumeAction::SeekToStartAndPlay,
        SourceKind::Asset => ResumeAction::ReplayAssetShell,
    }
}

/// Resumes playback or replays from EOS — all platforms, single entry.
///
/// Must be called WITHOUT the `shell` mutex held: this function locks `shell`
/// itself (in short scopes). Calling it from a context that already holds the
/// lock (e.g. inside `PlaybackEngine::run_on_gst`, which pre-locks) causes a
/// non-reentrant `parking_lot` self-deadlock that freezes the gst MainLoop.
pub fn resume_playing(
    shell: Arc<Mutex<PipelineShell>>,
    replay: &PlayReplayContext,
    swap: &PipelineSwapConfig,
    surface: &VideoSurface,
    overlay_ready: bool,
) -> Result<()> {
    // Guard the calling convention: try_lock returns None when the current
    // thread already holds `shell`, which would otherwise deadlock below.
    debug_assert!(
        shell.try_lock().is_some(),
        "resume_playing called with the shell lock already held (would self-deadlock)"
    );
    let kind = {
        let guard = shell.lock();
        guard.source_kind()
    };

    let at_eos = replay.at_eos.load(Ordering::SeqCst);
    let action = plan_resume_action(overlay_ready, at_eos, kind);

    if action == ResumeAction::DeferOverlay {
        log::info!("gst: deferring play until overlay is ready");
        return Ok(());
    }

    if at_eos {
        replay.at_eos.store(false, Ordering::SeqCst);
    }

    match action {
        ResumeAction::DeferOverlay => unreachable!(),
        ResumeAction::SetPlaying => {
            let guard = shell.lock();
            guard.set_state_sync(gst::State::Playing)?;
        }
        ResumeAction::SeekToStartAndPlay => {
            // Manual replay after EOS resets speed to 1.0 (looping keeps the
            // speed via EosLoopSeek, which is a different path).
            *replay.rate.lock() = 1.0;
            let guard = shell.lock();
            guard.seek_to_start_with_rate(1.0)?;
            guard.set_state_sync(gst::State::Playing)?;
        }
        ResumeAction::ReplayAssetShell => {
            // Manual asset replay after EOS resets speed to 1.0; the rebuilt
            // shell already starts at rate 1.0.
            *replay.rate.lock() = 1.0;
            let mut guard = shell.lock();
            replay_asset_shell(&mut guard, replay, swap, surface)?;
        }
    }

    #[cfg(target_os = "android")]
    android_refresh_after_playing(&shell, surface)?;

    Ok(())
}

#[cfg(target_os = "android")]
fn android_refresh_after_playing(
    shell: &Arc<Mutex<PipelineShell>>,
    surface: &VideoSurface,
) -> Result<()> {
    if let Some(handle) = *surface.stored_handle().lock() {
        let (width, height) = surface.cached_dimensions();
        let guard = shell.lock();
        refresh_mobile_overlay_on_gst(&guard, handle, width, height, "after Playing")?;
    }
    Ok(())
}

/// After an overlay bind completes, resume if the user already requested play.
///
/// Bind paths must call this (or [`resume_playing`] with `overlay_ready: true`) so
/// macOS / Win / Linux recover the same way Android / iOS do when `desired_playing`
/// was set before the native surface existed.
///
/// If load deferred PAUSED preroll (overlay was unbound), the pipeline may still be
/// at Ready — transition through Paused then Playing so sinks finish prepare-window-handle.
pub fn maybe_resume_after_overlay_bind(
    shell: Arc<Mutex<PipelineShell>>,
    replay: &PlayReplayContext,
    swap: &PipelineSwapConfig,
    surface: &VideoSurface,
) -> Result<()> {
    if !replay.desired_playing.load(Ordering::SeqCst) {
        log::debug!("gst: overlay bound — desired_playing=false, skip resume");
        return Ok(());
    }
    {
        let guard = shell.lock();
        let snap = guard.snapshot();
        if snap.current == gst::State::Ready
            && snap.pending == gst::State::VoidPending
            && snap.has_pending_media
        {
            log::info!("gst: overlay bound — PAUSED preroll before play");
            guard.set_state_sync(gst::State::Paused)?;
        }
    }
    log::info!("gst: overlay bound — resuming play (desired_playing=true)");
    resume_playing(shell, replay, swap, surface, true)
}

/// Pure gate: whether bind-complete should call [`resume_playing`].
pub(crate) fn should_resume_after_overlay_bind(desired_playing: bool) -> bool {
    desired_playing
}

/// Computes whether the surface is ready for play resume on the active platform.
pub fn overlay_ready_for_play(surface: &VideoSurface) -> bool {
    surface.is_overlay_bound_on_gst()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_table_overlay_not_ready_defers() {
        assert_eq!(
            plan_resume_action(false, false, SourceKind::Uri),
            ResumeAction::DeferOverlay
        );
        assert_eq!(
            plan_resume_action(false, true, SourceKind::Asset),
            ResumeAction::DeferOverlay
        );
    }

    #[test]
    fn gate_table_ready_not_eos_plays() {
        assert_eq!(
            plan_resume_action(true, false, SourceKind::Uri),
            ResumeAction::SetPlaying
        );
        assert_eq!(
            plan_resume_action(true, false, SourceKind::Asset),
            ResumeAction::SetPlaying
        );
    }

    #[test]
    fn gate_table_eos_uri_seeks() {
        assert_eq!(
            plan_resume_action(true, true, SourceKind::Uri),
            ResumeAction::SeekToStartAndPlay
        );
    }

    #[test]
    fn gate_table_eos_asset_replays_shell() {
        assert_eq!(
            plan_resume_action(true, true, SourceKind::Asset),
            ResumeAction::ReplayAssetShell
        );
    }

    #[test]
    fn should_resume_after_bind_follows_desired_playing() {
        assert!(!should_resume_after_overlay_bind(false));
        assert!(should_resume_after_overlay_bind(true));
    }

    #[test]
    fn overlay_ready_for_play_requires_bound_handle() {
        use parking_lot::Mutex;

        let surface = VideoSurface::new(Arc::new(Mutex::new(None)));
        assert!(!overlay_ready_for_play(&surface));
        surface.cache_handle(0x1000);
        // Cache alone is not a GStreamer bind on any platform; bind flags stay false.
        assert!(!overlay_ready_for_play(&surface));
    }

    // Regression: `PlaybackEngine::run_on_gst` pre-locks the shell, and calling
    // `pipeline_play` -> `resume_playing` from inside it re-locked the same
    // non-reentrant mutex, self-deadlocking the gst MainLoop (2nd open / resume
    // after pause hung). `resume_playing` must be called without the shell lock;
    // this verifies the guard rejects a caller-held lock instead of deadlocking.
    #[test]
    fn resume_playing_rejects_caller_held_shell_lock() {
        use crate::playback::bus::Emitter;
        use crate::playback::gst::{
            InternalAspectRatioMode, InternalVideoMetadata, InternalVideoOrientationConfig,
        };
        use crate::playback::shell::new_test_shell;
        use crate::playback::tracks::TrackCache;
        use gstreamer as gst;
        use std::sync::atomic::AtomicBool;

        let _ = gst::init();
        let shell = Arc::new(Mutex::new(new_test_shell(
            gst::Pipeline::new(),
            gst::ElementFactory::make("fakesink")
                .build()
                .expect("fakesink"),
            SourceKind::Uri,
            None,
        )));
        let replay = PlayReplayContext {
            desired_playing: Arc::new(AtomicBool::new(true)),
            at_eos: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(true)),
            rate: Arc::new(Mutex::new(1.0)),
        };
        let swap = PipelineSwapConfig {
            emitter: Arc::new(Mutex::new(None::<Emitter>)),
            looping: Arc::new(AtomicBool::new(false)),
            metadata: Arc::new(Mutex::new(InternalVideoMetadata::default())),
            track_cache: Arc::new(Mutex::new(TrackCache::default())),
            orientation: InternalVideoOrientationConfig::default(),
            aspect: InternalAspectRatioMode::default(),
        };
        let surface = VideoSurface::new(Arc::new(Mutex::new(None)));

        let held = shell.lock();
        let shell_for_call = shell.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = resume_playing(shell_for_call, &replay, &swap, &surface, true);
        }));
        drop(held);
        assert!(
            result.is_err(),
            "resume_playing must reject a caller-held shell lock (guard), not deadlock"
        );
    }

    // Regression: manual replay after EOS (SeekToStartAndPlay) resets the shared
    // rate to 1.0 (looping keeps its rate via a different path). The UI mirrors
    // this reset in PlaybackSession.play().
    #[test]
    fn manual_eos_replay_resets_rate_to_one() {
        use crate::playback::bus::Emitter;
        use crate::playback::gst::{
            InternalAspectRatioMode, InternalVideoMetadata, InternalVideoOrientationConfig,
        };
        use crate::playback::shell::new_test_shell;
        use crate::playback::tracks::TrackCache;
        use gstreamer as gst;
        use gstreamer::prelude::*;
        use std::sync::atomic::AtomicBool;

        let _ = gst::init();
        let pipeline = gst::Pipeline::new();
        let src = gst::ElementFactory::make("audiotestsrc")
            .property("is-live", false)
            .build()
            .expect("audiotestsrc");
        let sink = gst::ElementFactory::make("fakesink")
            .property("sync", false)
            .build()
            .expect("fakesink");
        pipeline.add_many([&src, &sink]).expect("add");
        src.link(&sink).expect("link");
        let shell = Arc::new(Mutex::new(new_test_shell(
            pipeline,
            sink,
            SourceKind::Uri,
            None,
        )));
        // Preroll to PLAYING so the replay seek is valid, then release the lock
        // (resume_playing locks internally).
        shell
            .lock()
            .set_state_sync(gst::State::Playing)
            .expect("to playing");

        let replay = PlayReplayContext {
            desired_playing: Arc::new(AtomicBool::new(true)),
            at_eos: Arc::new(AtomicBool::new(true)),
            running: Arc::new(AtomicBool::new(true)),
            rate: Arc::new(Mutex::new(2.0)),
        };
        let swap = PipelineSwapConfig {
            emitter: Arc::new(Mutex::new(None::<Emitter>)),
            looping: Arc::new(AtomicBool::new(false)),
            metadata: Arc::new(Mutex::new(InternalVideoMetadata::default())),
            track_cache: Arc::new(Mutex::new(TrackCache::default())),
            orientation: InternalVideoOrientationConfig::default(),
            aspect: InternalAspectRatioMode::default(),
        };
        let surface = VideoSurface::new(Arc::new(Mutex::new(None)));

        resume_playing(shell.clone(), &replay, &swap, &surface, true).expect("resume");
        assert_eq!(
            *replay.rate.lock(),
            1.0,
            "manual EOS replay must reset rate to 1.0"
        );
        let _ = shell.lock().set_state_sync(gst::State::Null);
    }
}
