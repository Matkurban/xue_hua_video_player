//! Unified play / EOS resume — single interface for all platforms.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::playback::replay::{replay_asset_shell, PlayReplayContext};
use crate::playback::shell::{PipelineShell, SourceKind};
use crate::playback::state::set_state_sync;
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
pub fn resume_playing(
    shell: Arc<Mutex<PipelineShell>>,
    replay: &PlayReplayContext,
    swap: &PipelineSwapConfig,
    surface: &VideoSurface,
    overlay_ready: bool,
) -> Result<()> {
    let kind = {
        let guard = shell.lock();
        guard.kind
    };

    let at_eos = replay.at_eos.load(Ordering::SeqCst);
    let action = plan_resume_action(overlay_ready, at_eos, kind);

    if action == ResumeAction::DeferOverlay {
        log::debug!("gst: deferring play until overlay is ready");
        return Ok(());
    }

    if at_eos {
        replay.at_eos.store(false, Ordering::SeqCst);
    }

    match action {
        ResumeAction::DeferOverlay => unreachable!(),
        ResumeAction::SetPlaying => {
            let guard = shell.lock();
            set_state_sync(&guard.pipeline, gst::State::Playing)?;
        }
        ResumeAction::SeekToStartAndPlay => {
            let guard = shell.lock();
            guard
                .pipeline
                .seek_simple(
                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                    gst::ClockTime::ZERO,
                )
                .map_err(|e| anyhow!("seek to start before play: {e}"))?;
            set_state_sync(&guard.pipeline, gst::State::Playing)?;
        }
        ResumeAction::ReplayAssetShell => {
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

/// Computes whether the surface is ready for play resume on the active platform.
pub fn overlay_ready_for_play(surface: &VideoSurface) -> bool {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        return surface.is_overlay_bound_on_gst();
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = surface;
        true
    }
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
    fn overlay_ready_for_play_desktop_always_true() {
        use parking_lot::Mutex;

        let surface = VideoSurface::new(Arc::new(Mutex::new(None)));
        assert!(overlay_ready_for_play(&surface));
    }
}
