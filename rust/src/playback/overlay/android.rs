//! Android VideoOverlay backend (`amcvideosink` / native window).

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::gst_runtime::spawn_on_gst_thread;
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
use crate::playback::state::{resume_or_replay_from_eos, set_state_sync};
use crate::video::{
    clear_overlay_window_handle, expose_overlay, set_overlay_render_rectangle,
    set_overlay_window_handle,
};

use super::preroll_gate::{decide_preroll_action, PipelineSnapshot, PrerollAction};
use super::video_overlay::VideoOverlayBackend;

/// Android overlay operations delegated from [`super::super::surface::VideoSurface`].
pub struct AndroidOverlayBackend;

impl VideoOverlayBackend for AndroidOverlayBackend {
    fn stored_handle(&self) -> &Mutex<Option<usize>> {
        unreachable!(
            "AndroidOverlayBackend is a stateless delegate; use VideoSurface stored handle"
        )
    }
}

pub fn cache_android_native_window(stored: &Mutex<Option<usize>>, handle: usize) -> Result<()> {
    if handle == 0 {
        if let Some(old) = stored.lock().take() {
            crate::platform_view_android::release_native_window(old);
        }
        return Ok(());
    }
    let mut guard = stored.lock();
    if let Some(old) = *guard {
        if old != handle {
            crate::platform_view_android::release_native_window(old);
        }
    }
    *guard = Some(handle);
    Ok(())
}

/// Rebinds the cached native window on the Gst thread (Android).
pub fn refresh_mobile_overlay_on_gst(
    shell: &PipelineShell,
    handle: usize,
    width: i32,
    height: i32,
    reason: &str,
) -> Result<()> {
    set_overlay_window_handle(&shell.video_sink, handle)?;
    if width > 0 && height > 0 {
        set_overlay_render_rectangle(&shell.video_sink, width, height);
    }
    expose_overlay(&shell.video_sink);
    crate::diag::logcat_info(&format!("gst: overlay refresh {reason} ({width}x{height})"));
    Ok(())
}

fn apply_mobile_overlay_on_gst(
    shell: &mut PipelineShell,
    handle: usize,
    width: i32,
    height: i32,
    play_intent: &OverlayPlayIntent,
    overlay_bound: &AtomicBool,
) -> Result<()> {
    refresh_mobile_overlay_on_gst(shell, handle, width, height, "surface bind")?;
    overlay_bound.store(true, Ordering::SeqCst);
    let (_, current, pending) = shell.pipeline.state(gst::ClockTime::ZERO);
    crate::diag::logcat_info(&format!(
        "gst: overlay applied on Gst thread — pipeline {current:?} pending {pending:?}"
    ));
    maybe_preroll_and_resume_play(shell, play_intent, handle, width, height)
}

fn maybe_preroll_and_resume_play(
    shell: &mut PipelineShell,
    play_intent: &OverlayPlayIntent,
    handle: usize,
    width: i32,
    height: i32,
) -> Result<()> {
    let want_play = play_intent
        .bundle
        .replay
        .desired_playing
        .load(Ordering::SeqCst);
    const OVERLAY_READY: bool = true;

    for _ in 0..4 {
        let snapshot = PipelineSnapshot::from_shell(shell);
        let action = decide_preroll_action(snapshot, want_play, OVERLAY_READY);
        match action {
            PrerollAction::Noop | PrerollAction::Defer => break,
            PrerollAction::PausePreroll => {
                crate::diag::logcat_info("gst: overlay bound — starting Paused preroll");
                set_state_sync(&shell.pipeline, gst::State::Paused)?;
                refresh_mobile_overlay_on_gst(
                    shell,
                    handle,
                    width,
                    height,
                    "after Paused preroll",
                )?;
            }
            PrerollAction::ResumePlaying => {
                if snapshot.pending != gst::State::VoidPending {
                    crate::diag::logcat_info(&format!(
                        "gst: overlay bind — pipeline pending {:?}, current {:?}",
                        snapshot.pending, snapshot.current
                    ));
                    crate::diag::logcat_info("gst: overlay bound — resuming play while pending");
                } else {
                    crate::diag::logcat_info(
                        "gst: overlay bound — resuming play (desired_playing=true)",
                    );
                }
                resume_or_replay_from_eos(shell, Some(&play_intent.bundle))?;
                refresh_mobile_overlay_on_gst(shell, handle, width, height, "after Playing")?;
                break;
            }
        }
    }
    Ok(())
}

pub fn schedule_mobile_overlay_apply(
    bundle: Arc<Mutex<PipelineShell>>,
    stored: Arc<Mutex<Option<usize>>>,
    overlay_bound: Arc<AtomicBool>,
    width: i32,
    height: i32,
    play_intent: OverlayPlayIntent,
) {
    spawn_on_gst_thread(move || {
        let mut guard = bundle.lock();
        let Some(handle) = *stored.lock() else {
            overlay_bound.store(false, Ordering::SeqCst);
            return;
        };
        if let Err(e) = apply_mobile_overlay_on_gst(
            &mut guard,
            handle,
            width,
            height,
            &play_intent,
            &overlay_bound,
        ) {
            overlay_bound.store(false, Ordering::SeqCst);
            crate::diag::logcat_error(&format!("mobile overlay apply: {e:#}"));
        }
    });
}
