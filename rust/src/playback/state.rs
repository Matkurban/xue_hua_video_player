use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer::StateChangeSuccess;
use parking_lot::Mutex;

use crate::playback::replay::{replay_asset_shell, PlayReplayContext};
use crate::playback::shell::{PipelineShell, SourceKind};
use crate::playback::surface::VideoSurface;
use crate::playback::switch::PipelineSwapConfig;

const DEFAULT_STATE_TIMEOUT: gst::ClockTime = gst::ClockTime::from_seconds(10);

/// Sets pipeline/element state and waits until the transition completes.
pub fn set_state_sync(element: &impl IsA<gst::Element>, target: gst::State) -> Result<()> {
    set_state_sync_timeout(element, target, DEFAULT_STATE_TIMEOUT)
}

pub fn set_state_sync_timeout(
    element: &impl IsA<gst::Element>,
    target: gst::State,
    timeout: gst::ClockTime,
) -> Result<()> {
    let element = element.upcast_ref::<gst::Element>();
    let change = element.set_state(target).map_err(|e| {
        let msg = format!("set_state({target:?}) failed: {e}");
        log::error!("{msg}");
        #[cfg(target_os = "android")]
        crate::diag::logcat_error(&format!("gst: {msg}"));
        anyhow!("{msg}")
    })?;
    if matches!(change, StateChangeSuccess::Success) {
        return Ok(());
    }
    let (ret, current, _pending) = element.state(Some(timeout));
    ret.map_err(|e| {
        let msg = format!("get_state after set_state({target:?}) failed: {e}");
        log::error!("{msg}");
        #[cfg(target_os = "android")]
        crate::diag::logcat_error(&format!("gst: {msg}"));
        anyhow!("{msg}")
    })?;
    if current != target {
        let msg = format!(
            "element failed to change state to {target:?} (current {current:?}) within {timeout:?}"
        );
        log::error!("{msg}");
        #[cfg(target_os = "android")]
        crate::diag::logcat_error(&format!("gst: {msg}"));
        return Err(anyhow!("{msg}"));
    }
    Ok(())
}

/// Resumes or replays from EOS using the correct adapter for the active shell.
pub fn resume_or_replay_from_eos(
    shell: &mut PipelineShell,
    replay: Option<&PlayReplayContext>,
    swap: Option<&PipelineSwapConfig>,
    surface: Option<&VideoSurface>,
) -> Result<()> {
    #[cfg(target_os = "ios")]
    if let Some(surface) = surface {
        if !surface.is_overlay_bound_on_gst() {
            log::debug!("gst: deferring play until iOS layer is attached");
            return Ok(());
        }
        log::debug!("gst: PLAYING resume delegated to IosOverlaySession");
        return Ok(());
    }

    let replay = replay.ok_or_else(|| anyhow!("resume_or_replay_from_eos requires PlayReplayContext"))?;

    if !replay.at_eos.swap(false, Ordering::SeqCst) {
        return set_state_sync(&shell.pipeline, gst::State::Playing);
    }
    match shell.kind {
        SourceKind::Uri => {
            shell
                .pipeline
                .seek_simple(
                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                    gst::ClockTime::ZERO,
                )
                .map_err(|e| anyhow!("seek to start before play: {e}"))?;
            set_state_sync(&shell.pipeline, gst::State::Playing)
        }
        SourceKind::Asset => {
            let swap = swap.ok_or_else(|| anyhow!("asset EOS replay requires PipelineSwapConfig"))?;
            let surface = surface.ok_or_else(|| anyhow!("asset EOS replay requires VideoSurface"))?;
            replay_asset_shell(shell, replay, swap, surface)
        }
    }
}
