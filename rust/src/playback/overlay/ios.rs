//! iOS bus-facing overlay backend — thin adapter to [`super::ios_session::IosOverlaySession`].

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::overlay::ios_session::{IosIdleWork, IosOverlaySession};
use crate::playback::replay::{OverlayPlayIntent, PlayReplayContext};
use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;
use crate::playback::switch::PipelineSwapConfig;
use crate::video::ios_layer::IosLayerAttachOutcome;

/// Shared state for iOS layer attach retries on the Gst bus (`READY → PAUSED`).
pub struct IosLayerBackend {
    pub shell: Arc<Mutex<PipelineShell>>,
    ios_session: IosOverlaySession,
    pub stored: Arc<Mutex<Option<usize>>>,
    pub overlay_sink: Arc<Mutex<gst::Element>>,
    replay: PlayReplayContext,
    swap: PipelineSwapConfig,
    surface: VideoSurface,
    ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
}

impl IosLayerBackend {
    pub fn from_engine(
        surface: &VideoSurface,
        shell: Arc<Mutex<PipelineShell>>,
        replay: PlayReplayContext,
        swap: PipelineSwapConfig,
    ) -> Self {
        Self {
            shell,
            ios_session: surface.ios_session(),
            stored: surface.stored_handle(),
            overlay_sink: surface
                .overlay_sink_slot()
                .expect("ios overlay sink slot")
                .clone(),
            replay,
            swap,
            surface: surface.clone_for_switch(),
            ios_layer_bus_slot: surface.ios_layer_bus_slot(),
        }
    }

    pub fn is_overlay_bound(&self) -> bool {
        self.ios_session.is_bound()
    }

    pub fn set_pending_play_after_overlay(&self) {
        self.ios_session.set_pending_play_after_overlay(true);
    }

    pub fn set_buffering_active(&self, active: bool) {
        self.ios_session.set_buffering_active(active);
    }

    fn overlay_play_intent(&self) -> OverlayPlayIntent {
        OverlayPlayIntent {
            replay: self.replay.clone(),
            swap: self.swap.clone_for_async(),
        }
    }

    fn idle_work(&self) -> IosIdleWork {
        let play_intent = self.overlay_play_intent();
        self.ios_session.idle_work(
            self.shell.clone(),
            self.stored.clone(),
            self.surface.clone_for_switch(),
            play_intent,
            self.ios_layer_bus_slot.clone(),
        )
    }

    pub fn schedule_apply(&self) {
        self.ios_session.schedule_apply(self.idle_work());
    }

    pub fn schedule_attach(&self) {
        self.ios_session.schedule_attach(self.idle_work());
    }

    /// Routes PLAYING resume through idle [`IosOverlaySession::apply_target_state`].
    pub fn request_playing_resume(&self) {
        self.schedule_apply();
    }

    pub fn try_attach(&self) {
        let play_intent = self.overlay_play_intent();
        match self.ios_session.request_attach(
            self.shell.clone(),
            self.stored.clone(),
            self.surface.clone_for_switch(),
            play_intent,
            "READY→PAUSED",
            self.ios_session.overlay_generation().load(Ordering::SeqCst),
            self.ios_layer_bus_slot.clone(),
        ) {
            Ok(IosLayerAttachOutcome::LayerNotReady) => {
                log::debug!("gst: ios layer attach on READY→PAUSED: layer not ready yet");
            }
            Ok(IosLayerAttachOutcome::Scheduled) => {}
            Ok(IosLayerAttachOutcome::Skipped) => {}
            Err(e) => {
                log::debug!("gst: ios layer attach on READY→PAUSED: {e:#}");
            }
        }
    }

    pub fn update_swap(&mut self, swap: PipelineSwapConfig) {
        self.swap = swap;
    }
}
