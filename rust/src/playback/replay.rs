//! Play/replay context shared between overlay attach and EOS replay paths.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;
use crate::playback::switch::{switch_asset_shell, PipelineSwapConfig};

/// Playback intent atomics for overlay bind and resume paths.
#[derive(Clone)]
pub struct PlayReplayContext {
    pub desired_playing: Arc<AtomicBool>,
    pub at_eos: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
}

/// Unified play intent for mobile overlay bind (Android + iOS).
#[derive(Clone)]
pub struct OverlayPlayIntent {
    pub replay: PlayReplayContext,
    pub swap: PipelineSwapConfig,
}

impl OverlayPlayIntent {
    pub fn clone_for_async(&self) -> Self {
        Self {
            replay: self.replay.clone(),
            swap: self.swap.clone_for_async(),
        }
    }
}

/// Replays an asset from EOS by tearing down and rebuilding the shell (fresh decodebin).
pub fn replay_asset_shell(
    shell: &mut PipelineShell,
    replay: &PlayReplayContext,
    swap: &PipelineSwapConfig,
    surface: &VideoSurface,
) -> Result<()> {
    let key = shell
        .asset_key()
        .ok_or_else(|| anyhow!("asset replay requested but asset_key missing"))?
        .to_string();
    switch_asset_shell(shell, &key, swap, replay, surface)?;
    shell.set_state_sync(gst::State::Playing)?;
    #[cfg(target_os = "android")]
    crate::diag::logcat_info("gst: AppSrc replay from EOS (shell reload)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::bus::Emitter;
    use crate::playback::gst::{
        InternalAspectRatioMode, InternalVideoMetadata, InternalVideoOrientationConfig,
    };
    use crate::playback::surface::VideoSurface;
    use crate::playback::tracks::TrackCache;

    fn sample_intent() -> OverlayPlayIntent {
        let desired = Arc::new(AtomicBool::new(true));
        let at_eos = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        OverlayPlayIntent {
            replay: PlayReplayContext {
                desired_playing: desired,
                at_eos,
                running,
            },
            swap: PipelineSwapConfig {
                emitter: Arc::new(Mutex::new(None)),
                looping: Arc::new(AtomicBool::new(false)),
                metadata: Arc::new(Mutex::new(InternalVideoMetadata::default())),
                track_cache: Arc::new(Mutex::new(TrackCache::default())),
                orientation: InternalVideoOrientationConfig::default(),
                aspect: InternalAspectRatioMode::default(),
            },
        }
    }

    #[test]
    fn intent_clone_for_async_shares_atomic_identity() {
        let intent = sample_intent();
        let cloned = intent.clone_for_async();
        assert!(Arc::ptr_eq(
            &intent.replay.desired_playing,
            &cloned.replay.desired_playing
        ));
    }
}
