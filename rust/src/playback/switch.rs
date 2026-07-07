use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::media::ResolvedSource;
use crate::playback::bus::Emitter;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::playback::overlay::assign_overlay_sink;
#[cfg(target_os = "android")]
use crate::playback::overlay::refresh_mobile_overlay_on_gst;
#[cfg(target_os = "ios")]
use crate::playback::overlay::IosLayerBackend;
use crate::playback::overlay::{decide_preroll_action, PipelineSnapshot, PrerollAction};
use crate::playback::shell::{
    install_asset_shell, install_uri_shell, teardown_shell, wire_overlay_sync, PipelineShell,
    SourceKind,
};
use crate::playback::state::set_state_sync;
use crate::playback::surface::VideoSurface;
use crate::playback::tracks::TrackCache;
use crate::video::orientation::apply_orientation_to_playbin;
use crate::video::{
    info::InternalVideoMetadata,
    orientation::{InternalAspectRatioMode, InternalVideoOrientationConfig},
};

/// Shared state required when swapping the active [`PipelineShell`].
pub struct ShellTransition {
    pub emitter: Arc<Mutex<Option<Emitter>>>,
    pub looping: Arc<AtomicBool>,
    pub desired_playing: Arc<AtomicBool>,
    pub at_eos: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
    pub metadata: Arc<Mutex<InternalVideoMetadata>>,
    pub track_cache: Arc<Mutex<TrackCache>>,
    pub orientation: InternalVideoOrientationConfig,
    pub aspect: InternalAspectRatioMode,
    pub surface: VideoSurface,
}

impl ShellTransition {
    /// Clones shared handles for async overlay / replay callbacks.
    pub fn clone_for_async(&self) -> Self {
        Self {
            emitter: self.emitter.clone(),
            looping: self.looping.clone(),
            desired_playing: self.desired_playing.clone(),
            at_eos: self.at_eos.clone(),
            running: self.running.clone(),
            metadata: self.metadata.clone(),
            track_cache: self.track_cache.clone(),
            orientation: self.orientation,
            aspect: self.aspect,
            surface: self.surface.clone_for_switch(),
        }
    }
}

/// Rebuilds or reconfigures the pipeline shell for `resolved` and applies overlay/orientation.
pub fn switch_shell(
    shell: &mut PipelineShell,
    resolved: ResolvedSource,
    transition: &ShellTransition,
    #[cfg(target_os = "ios")] ios_layer_bus_slot: Option<&Arc<Mutex<Option<IosLayerBackend>>>>,
) -> Result<()> {
    match resolved {
        ResolvedSource::Uri(uri) => switch_uri_shell(
            shell,
            &uri,
            transition,
            #[cfg(target_os = "ios")]
            ios_layer_bus_slot,
        ),
        ResolvedSource::AppSrc(asset_key) => switch_asset_shell(
            shell,
            &asset_key,
            transition,
            #[cfg(target_os = "ios")]
            ios_layer_bus_slot,
        ),
    }
}

fn switch_uri_shell(
    shell: &mut PipelineShell,
    uri: &str,
    transition: &ShellTransition,
    #[cfg(target_os = "ios")] ios_layer_bus_slot: Option<&Arc<Mutex<Option<IosLayerBackend>>>>,
) -> Result<()> {
    if shell.kind != SourceKind::Uri {
        teardown_shell(shell);
        transition.surface.mark_shell_rebuilt();
        *shell = install_uri_shell(
            &transition.emitter,
            &transition.looping,
            &transition.desired_playing,
            &transition.at_eos,
            &transition.running,
            Some(transition.metadata.clone()),
            Some(transition.track_cache.clone()),
            #[cfg(target_os = "ios")]
            ios_layer_bus_slot,
        )?;
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let overlay_sink = transition.surface.overlay_sink_slot().cloned();
            wire_overlay_sync(shell, transition.surface.stored_handle(), overlay_sink);
            if let Some(slot) = transition.surface.overlay_sink_slot() {
                assign_overlay_sink(slot, &shell.video_sink);
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        wire_overlay_sync(shell, transition.surface.stored_handle());
    }
    transition.surface.rebind_cached_overlay(shell)?;
    transition.aspect.apply_to_sink(&shell.video_sink);
    apply_orientation_to_playbin(
        shell.pipeline.upcast_ref::<gst::Element>(),
        transition.orientation,
    )?;
    let has_overlay = transition.surface.overlay_ready_for_preroll();
    pipeline_set_uri(
        shell,
        uri,
        &transition.at_eos,
        has_overlay,
        &transition.surface,
    )
}

pub(crate) fn switch_asset_shell(
    shell: &mut PipelineShell,
    asset_key: &str,
    transition: &ShellTransition,
    #[cfg(target_os = "ios")] ios_layer_bus_slot: Option<&Arc<Mutex<Option<IosLayerBackend>>>>,
) -> Result<()> {
    teardown_shell(shell);
    transition.surface.mark_shell_rebuilt();
    *shell = install_asset_shell(
        asset_key,
        &transition.emitter,
        &transition.looping,
        &transition.desired_playing,
        &transition.at_eos,
        &transition.running,
        Some(transition.metadata.clone()),
        #[cfg(target_os = "ios")]
        ios_layer_bus_slot,
    )?;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let overlay_sink = transition.surface.overlay_sink_slot().cloned();
        wire_overlay_sync(shell, transition.surface.stored_handle(), overlay_sink);
        if let Some(slot) = transition.surface.overlay_sink_slot() {
            assign_overlay_sink(slot, &shell.video_sink);
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    wire_overlay_sync(shell, transition.surface.stored_handle());
    transition.surface.rebind_cached_overlay(shell)?;
    transition.aspect.apply_to_sink(&shell.video_sink);
    transition.at_eos.store(false, Ordering::SeqCst);
    preroll_asset_shell(
        shell,
        transition.surface.overlay_ready_for_preroll(),
        &transition.surface,
    )
}

/// Maps platform overlay readiness to gate input for URI/asset **load** preroll.
fn gate_overlay_ready_for_load(surface_overlay_ready: bool) -> bool {
    #[cfg(target_os = "android")]
    {
        return surface_overlay_ready;
    }
    #[cfg(target_os = "ios")]
    {
        let _ = surface_overlay_ready;
        return false;
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        // Desktop/macOS: preroll when handle is not cached yet (see pipeline_set_uri).
        !surface_overlay_ready
    }
}

fn apply_load_preroll(
    shell: &PipelineShell,
    surface_overlay_ready: bool,
    surface: &VideoSurface,
    defer_log: &str,
) -> Result<()> {
    let gate_ready = gate_overlay_ready_for_load(surface_overlay_ready);
    let snapshot = PipelineSnapshot::from_shell(shell);
    match decide_preroll_action(snapshot, false, gate_ready) {
        PrerollAction::PausePreroll => {
            set_state_sync(&shell.pipeline, gst::State::Paused)?;
            #[cfg(target_os = "android")]
            if let Some(handle) = *surface.stored_handle().lock() {
                let (width, height) = surface.cached_dimensions();
                refresh_mobile_overlay_on_gst(
                    shell,
                    handle,
                    width,
                    height,
                    "after Paused preroll",
                )?;
            }
        }
        PrerollAction::Defer => {
            #[cfg(target_os = "android")]
            crate::diag::logcat_info(defer_log);
            #[cfg(target_os = "ios")]
            log::info!("{defer_log}");
        }
        PrerollAction::Noop | PrerollAction::ResumePlaying => {}
    }
    Ok(())
}

fn preroll_asset_shell(
    shell: &PipelineShell,
    overlay_ready: bool,
    surface: &VideoSurface,
) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        return apply_load_preroll(
            shell,
            overlay_ready,
            surface,
            "gst: deferring asset Paused preroll until Android overlay is bound",
        );
    }
    #[cfg(target_os = "ios")]
    {
        let _ = shell;
        if overlay_ready {
            log::debug!("gst: ios layer attach deferred to IosOverlaySession after load");
        } else {
            log::info!("gst: deferring asset load until iOS host view is cached");
        }
        return Ok(());
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let snapshot = PipelineSnapshot::from_shell(shell);
        if decide_preroll_action(snapshot, false, true) == PrerollAction::PausePreroll {
            set_state_sync(&shell.pipeline, gst::State::Paused)?;
        }
        Ok(())
    }
}

fn pipeline_set_uri(
    shell: &PipelineShell,
    uri: &str,
    at_eos: &AtomicBool,
    overlay_ready: bool,
    surface: &VideoSurface,
) -> Result<()> {
    let pipeline = &shell.pipeline;
    at_eos.store(false, Ordering::SeqCst);
    set_state_sync(pipeline, gst::State::Ready)?;
    pipeline.set_property("uri", uri);
    #[cfg(target_os = "android")]
    {
        return apply_load_preroll(
            shell,
            overlay_ready,
            surface,
            "gst: deferring URI Paused preroll until Android overlay is bound",
        );
    }
    #[cfg(target_os = "ios")]
    {
        let _ = surface;
        if overlay_ready {
            log::debug!("gst: ios layer attach deferred to IosOverlaySession after setUri");
        } else {
            log::info!("gst: deferring URI load until iOS host view is cached");
        }
        return Ok(());
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        apply_load_preroll(shell, overlay_ready, surface, "")
    }
}
