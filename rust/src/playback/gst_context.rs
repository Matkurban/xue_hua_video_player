//! Bundled Gst-thread context for load, play, switch, and overlay paths.

use std::sync::{atomic::AtomicBool, Arc};

use parking_lot::Mutex;

use crate::playback::bus::Emitter;
use crate::playback::replay::{OverlayPlayIntent, PlayReplayContext};
use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;
use crate::playback::switch::PipelineSwapConfig;
use crate::playback::tracks::TrackCache;
use crate::video::info::InternalVideoMetadata;
use crate::video::orientation::{InternalAspectRatioMode, InternalVideoOrientationConfig};

/// Live engine-owned Gst bundle — `shell` and `surface` are canonical.
pub struct PlaybackGstContext {
    pub shell: Arc<Mutex<PipelineShell>>,
    pub surface: VideoSurface,
    pub replay: PlayReplayContext,
    emitter: Arc<Mutex<Option<Emitter>>>,
    looping: Arc<AtomicBool>,
    metadata: Arc<Mutex<InternalVideoMetadata>>,
    track_cache: Arc<Mutex<TrackCache>>,
    orientation: Arc<Mutex<InternalVideoOrientationConfig>>,
    aspect_mode: Arc<Mutex<InternalAspectRatioMode>>,
}

/// Async-safe snapshot passed into Gst thread closures.
pub struct PlaybackGstAsyncSnapshot {
    pub shell: Arc<Mutex<PipelineShell>>,
    pub surface: VideoSurface,
    pub replay: PlayReplayContext,
    pub swap: PipelineSwapConfig,
}

impl Clone for PlaybackGstAsyncSnapshot {
    fn clone(&self) -> Self {
        Self {
            shell: self.shell.clone(),
            surface: self.surface.clone_for_switch(),
            replay: self.replay.clone(),
            swap: self.swap.clone_for_async(),
        }
    }
}

impl PlaybackGstContext {
    pub fn new(
        shell: Arc<Mutex<PipelineShell>>,
        surface: VideoSurface,
        replay: PlayReplayContext,
        emitter: Arc<Mutex<Option<Emitter>>>,
        looping: Arc<AtomicBool>,
        metadata: Arc<Mutex<InternalVideoMetadata>>,
        track_cache: Arc<Mutex<TrackCache>>,
        orientation: Arc<Mutex<InternalVideoOrientationConfig>>,
        aspect_mode: Arc<Mutex<InternalAspectRatioMode>>,
    ) -> Self {
        Self {
            shell,
            surface,
            replay,
            emitter,
            looping,
            metadata,
            track_cache,
            orientation,
            aspect_mode,
        }
    }

    pub fn swap_config(&self) -> PipelineSwapConfig {
        PipelineSwapConfig {
            emitter: self.emitter.clone(),
            looping: self.looping.clone(),
            metadata: self.metadata.clone(),
            track_cache: self.track_cache.clone(),
            orientation: *self.orientation.lock(),
            aspect: *self.aspect_mode.lock(),
        }
    }

    pub fn overlay_intent(&self) -> OverlayPlayIntent {
        OverlayPlayIntent {
            replay: self.replay.clone(),
            swap: self.swap_config(),
        }
    }

    pub fn clone_for_async(&self) -> PlaybackGstAsyncSnapshot {
        PlaybackGstAsyncSnapshot {
            shell: self.shell.clone(),
            surface: self.surface.clone_for_switch(),
            replay: self.replay.clone(),
            swap: self.swap_config().clone_for_async(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    use gstreamer as gst;

    use crate::playback::shell::SourceKind;

    fn sample_context_with_swap(
        orientation: InternalVideoOrientationConfig,
        aspect: InternalAspectRatioMode,
    ) -> PlaybackGstContext {
        let _ = gst::init();
        let shell = crate::playback::shell::new_test_shell(
            gst::Pipeline::new(),
            gst::ElementFactory::make("fakesink")
                .build()
                .expect("fakesink"),
            SourceKind::Uri,
            None,
        );
        let desired = Arc::new(AtomicBool::new(true));
        let at_eos = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let overlay = Arc::new(Mutex::new(None));
        let surface = VideoSurface::new(overlay);
        PlaybackGstContext::new(
            Arc::new(Mutex::new(shell)),
            surface,
            PlayReplayContext {
                desired_playing: desired,
                at_eos,
                running,
            },
            Arc::new(Mutex::new(None)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(InternalVideoMetadata::default())),
            Arc::new(Mutex::new(TrackCache::default())),
            Arc::new(Mutex::new(orientation)),
            Arc::new(Mutex::new(aspect)),
        )
    }

    fn sample_context() -> PlaybackGstContext {
        sample_context_with_swap(
            InternalVideoOrientationConfig::default(),
            InternalAspectRatioMode::default(),
        )
    }

    #[test]
    fn clone_for_async_snapshots_orientation_and_aspect_from_locks() {
        let ctx = sample_context_with_swap(
            InternalVideoOrientationConfig {
                rotate_degrees: 90,
                ..Default::default()
            },
            InternalAspectRatioMode::Fill,
        );

        let snap = ctx.clone_for_async();
        assert_eq!(snap.swap.orientation.rotate_degrees, 90);
        assert_eq!(snap.swap.aspect, InternalAspectRatioMode::Fill);
    }

    #[test]
    fn clone_for_async_shares_replay_atomic_identity() {
        let ctx = sample_context();
        let snap = ctx.clone_for_async();
        assert!(Arc::ptr_eq(
            &ctx.replay.desired_playing,
            &snap.replay.desired_playing
        ));
    }

    #[test]
    fn overlay_intent_uses_live_swap_snapshot() {
        let ctx = sample_context_with_swap(
            InternalVideoOrientationConfig::default(),
            InternalAspectRatioMode::Fit,
        );
        ctx.replay.desired_playing.store(false, Ordering::SeqCst);

        let intent = ctx.overlay_intent();
        assert!(!intent.replay.desired_playing.load(Ordering::SeqCst));
        assert_eq!(intent.swap.aspect, InternalAspectRatioMode::Fit);
        assert!(Arc::ptr_eq(
            &ctx.replay.desired_playing,
            &intent.replay.desired_playing
        ));
    }
}
