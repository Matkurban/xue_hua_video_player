//! iOS bus-facing overlay backend — thin adapter to [`super::ios_session::IosOverlaySession`].

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::gst_context::PlaybackGstContext;
use super::session::{IosIdleWork, IosOverlaySession};
use crate::playback::shell::PipelineShell;
use crate::platform::ios::layer::IosLayerAttachOutcome;

/// Shared state for iOS layer attach retries on the Gst bus (`READY → PAUSED`).
pub struct IosLayerBackend {
    ctx: Arc<PlaybackGstContext>,
    ios_session: IosOverlaySession,
    pub overlay_sink: Arc<Mutex<gst::Element>>,
    ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
}

impl IosLayerBackend {
    pub fn from_context(ctx: Arc<PlaybackGstContext>) -> Self {
        let surface = &ctx.surface;
        Self {
            ios_session: surface.ios_session(),
            overlay_sink: surface
                .overlay_sink_slot()
                .expect("ios overlay sink slot")
                .clone(),
            ios_layer_bus_slot: surface.ios_layer_bus_slot(),
            ctx,
        }
    }

    pub fn shell(&self) -> Arc<Mutex<PipelineShell>> {
        self.ctx.shell.clone()
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

    fn idle_work(&self) -> IosIdleWork {
        let play_intent = self.ctx.overlay_intent().clone_for_async();
        self.ios_session.idle_work(
            self.ctx.shell.clone(),
            self.ctx.surface.stored_handle(),
            self.ctx.surface.clone_for_switch(),
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
        let play_intent = self.ctx.overlay_intent().clone_for_async();
        match self.ios_session.request_attach(
            self.ctx.shell.clone(),
            self.ctx.surface.stored_handle(),
            self.ctx.surface.clone_for_switch(),
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
}
