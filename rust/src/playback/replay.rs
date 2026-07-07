//! Play/replay context shared between overlay attach and EOS replay paths.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::shell::PipelineShell;
use crate::playback::state::set_state_sync;
use crate::playback::switch::{switch_asset_shell, ShellTransition};

/// Playback intent atomics for overlay bind and resume paths.
#[derive(Clone)]
pub struct PlayReplayContext {
    pub desired_playing: Arc<AtomicBool>,
    pub at_eos: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
}

/// Shell transition + replay atomics for async overlay callbacks and asset EOS replay.
pub struct OverlayPlayBundle {
    pub replay: PlayReplayContext,
    pub shell: ShellTransition,
}

impl OverlayPlayBundle {
    /// Clones shared handles for async iOS layer attach completion callbacks.
    pub fn clone_for_async(&self) -> Self {
        Self {
            replay: self.replay.clone(),
            shell: self.shell.clone_for_async(),
        }
    }
}

/// Unified play intent for mobile overlay bind (Android + iOS).
pub struct OverlayPlayIntent {
    pub bundle: OverlayPlayBundle,
}

impl OverlayPlayIntent {
    pub fn clone_for_async(&self) -> Self {
        Self {
            bundle: self.bundle.clone_for_async(),
        }
    }
}

/// Replays an asset from EOS by tearing down and rebuilding the shell (fresh decodebin).
pub fn replay_asset_shell(
    shell: &mut PipelineShell,
    bundle: &OverlayPlayBundle,
    #[cfg(target_os = "ios")] ios_layer_bus_slot: Option<
        &Arc<Mutex<Option<crate::playback::overlay::IosLayerBackend>>>,
    >,
) -> Result<()> {
    let key = shell
        .asset_key
        .clone()
        .ok_or_else(|| anyhow!("asset replay requested but asset_key missing"))?;
    switch_asset_shell(
        shell,
        &key,
        &bundle.shell,
        #[cfg(target_os = "ios")]
        ios_layer_bus_slot,
    )?;
    set_state_sync(&shell.pipeline, gst::State::Playing)?;
    #[cfg(target_os = "android")]
    crate::diag::logcat_info("gst: AppSrc replay from EOS (shell reload)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::surface::VideoSurface;
    use std::sync::atomic::Ordering;

    fn sample_bundle() -> OverlayPlayBundle {
        let desired = Arc::new(AtomicBool::new(true));
        let at_eos = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let emitter = Arc::new(Mutex::new(None));
        let looping = Arc::new(AtomicBool::new(false));
        let metadata = Arc::new(Mutex::new(
            crate::video::info::InternalVideoMetadata::default(),
        ));
        let track_cache = Arc::new(Mutex::new(crate::playback::tracks::TrackCache::default()));
        let surface = VideoSurface::new(Arc::new(Mutex::new(None)));
        OverlayPlayBundle {
            replay: PlayReplayContext {
                desired_playing: desired.clone(),
                at_eos: at_eos.clone(),
                running: running.clone(),
            },
            shell: ShellTransition {
                emitter,
                looping,
                desired_playing: desired,
                at_eos,
                running,
                metadata,
                track_cache,
                orientation: crate::video::orientation::InternalVideoOrientationConfig::default(),
                aspect: crate::video::orientation::InternalAspectRatioMode::default(),
                surface,
            },
        }
    }

    #[test]
    fn bundle_clone_for_async_shares_atomic_identity() {
        let bundle = sample_bundle();
        let cloned = bundle.clone_for_async();
        assert!(Arc::ptr_eq(
            &bundle.replay.desired_playing,
            &cloned.replay.desired_playing
        ));
        assert!(Arc::ptr_eq(
            &bundle.shell.desired_playing,
            &cloned.shell.desired_playing
        ));
        assert!(Arc::ptr_eq(
            &bundle.replay.desired_playing,
            &cloned.shell.desired_playing
        ));
    }

    #[test]
    fn bundle_clone_for_async_clones_surface_state() {
        let bundle = sample_bundle();
        bundle.shell.surface.cache_handle(7);
        let cloned = bundle.clone_for_async();
        assert_eq!(*cloned.shell.surface.stored_handle().lock(), Some(7));
    }

    #[test]
    fn intent_clone_for_async_preserves_playback_flags() {
        let bundle = sample_bundle();
        bundle.replay.desired_playing.store(true, Ordering::SeqCst);
        let intent = OverlayPlayIntent { bundle };
        let cloned = intent.clone_for_async();
        assert!(cloned.bundle.replay.desired_playing.load(Ordering::SeqCst));
    }
}
