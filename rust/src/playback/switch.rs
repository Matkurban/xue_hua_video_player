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
use crate::playback::overlay::{platform_load_preroll_policy, LoadPrerollPolicy};
use crate::playback::replay::PlayReplayContext;
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

/// Pipeline-only metadata for URI ↔ asset shell swaps (no replay atomics, no surface).
#[derive(Clone)]
pub struct PipelineSwapConfig {
    pub emitter: Arc<Mutex<Option<Emitter>>>,
    pub looping: Arc<AtomicBool>,
    pub metadata: Arc<Mutex<InternalVideoMetadata>>,
    pub track_cache: Arc<Mutex<TrackCache>>,
    pub orientation: InternalVideoOrientationConfig,
    pub aspect: InternalAspectRatioMode,
}

impl PipelineSwapConfig {
    pub fn clone_for_async(&self) -> Self {
        Self {
            emitter: self.emitter.clone(),
            looping: self.looping.clone(),
            metadata: self.metadata.clone(),
            track_cache: self.track_cache.clone(),
            orientation: self.orientation,
            aspect: self.aspect,
        }
    }
}

/// Rebuilds or reconfigures the pipeline shell for `resolved` and applies overlay/orientation.
pub fn switch_shell(
    shell: &mut PipelineShell,
    resolved: ResolvedSource,
    swap: &PipelineSwapConfig,
    replay: &PlayReplayContext,
    surface: &VideoSurface,
) -> Result<()> {
    match resolved {
        ResolvedSource::Uri(uri) => switch_uri_shell(shell, &uri, swap, replay, surface),
        ResolvedSource::AppSrc(asset_key) => {
            switch_asset_shell(shell, &asset_key, swap, replay, surface)
        }
    }
}

fn switch_uri_shell(
    shell: &mut PipelineShell,
    uri: &str,
    swap: &PipelineSwapConfig,
    replay: &PlayReplayContext,
    surface: &VideoSurface,
) -> Result<()> {
    if shell.kind != SourceKind::Uri {
        teardown_shell(shell);
        surface.mark_shell_rebuilt();
        *shell = install_uri_shell(
            &swap.emitter,
            &swap.looping,
            replay,
            Some(swap.metadata.clone()),
            Some(swap.track_cache.clone()),
            surface,
        )?;
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let overlay_sink = surface.overlay_sink_slot().cloned();
            wire_overlay_sync(shell, surface.stored_handle(), overlay_sink);
            if let Some(slot) = surface.overlay_sink_slot() {
                assign_overlay_sink(slot, &shell.video_sink);
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        wire_overlay_sync(shell, surface.stored_handle());
    }
    surface.rebind_cached_overlay(shell)?;
    swap.aspect.apply_to_sink(&shell.video_sink);
    apply_orientation_to_playbin(
        shell.pipeline.upcast_ref::<gst::Element>(),
        swap.orientation,
    )?;
    let has_overlay = surface.overlay_ready_for_preroll();
    pipeline_set_uri(shell, uri, replay, has_overlay, surface)
}

pub(crate) fn switch_asset_shell(
    shell: &mut PipelineShell,
    asset_key: &str,
    swap: &PipelineSwapConfig,
    replay: &PlayReplayContext,
    surface: &VideoSurface,
) -> Result<()> {
    teardown_shell(shell);
    surface.mark_shell_rebuilt();
    *shell = install_asset_shell(
        asset_key,
        &swap.emitter,
        &swap.looping,
        replay,
        Some(swap.metadata.clone()),
        surface,
    )?;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let overlay_sink = surface.overlay_sink_slot().cloned();
        wire_overlay_sync(shell, surface.stored_handle(), overlay_sink);
        if let Some(slot) = surface.overlay_sink_slot() {
            assign_overlay_sink(slot, &shell.video_sink);
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    wire_overlay_sync(shell, surface.stored_handle());
    surface.rebind_cached_overlay(shell)?;
    swap.aspect.apply_to_sink(&shell.video_sink);
    replay.at_eos.store(false, Ordering::SeqCst);
    preroll_asset_shell(
        shell,
        surface.overlay_ready_for_preroll(),
        surface,
        "gst: deferring asset Paused preroll until Android overlay is bound",
    )
}

fn preroll_asset_shell(
    shell: &PipelineShell,
    overlay_ready: bool,
    surface: &VideoSurface,
    defer_log: &str,
) -> Result<()> {
    platform_load_preroll_policy().apply_load_preroll(shell, overlay_ready, surface, defer_log)
}

fn pipeline_set_uri(
    shell: &PipelineShell,
    uri: &str,
    replay: &PlayReplayContext,
    overlay_ready: bool,
    surface: &VideoSurface,
) -> Result<()> {
    let pipeline = &shell.pipeline;
    replay.at_eos.store(false, Ordering::SeqCst);
    set_state_sync(pipeline, gst::State::Ready)?;
    pipeline.set_property("uri", uri);
    platform_load_preroll_policy().apply_load_preroll(
        shell,
        overlay_ready,
        surface,
        "gst: deferring URI Paused preroll until Android overlay is bound",
    )
}
