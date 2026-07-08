use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use parking_lot::Mutex;

use crate::media::ResolvedSource;
use crate::playback::bus::Emitter;
use crate::playback::gst::{
    InternalAspectRatioMode, InternalVideoMetadata, InternalVideoOrientationConfig,
};
use crate::playback::overlay::OverlaySession;
use crate::playback::replay::PlayReplayContext;
use crate::playback::shell::{
    install_asset_shell, install_uri_shell, teardown_shell, wire_overlay_sync, PipelineShell,
};
#[cfg(target_os = "android")]
use crate::playback::sink::OverlaySizeSync;
use crate::playback::surface::VideoSurface;
use crate::playback::tracks::TrackCache;

/// Pipeline-only metadata for URI ↔ asset shell swaps (no replay atomics, no surface).
#[derive(Clone)]
pub struct PipelineSwapConfig {
    pub emitter: Arc<Mutex<Option<Emitter>>>,
    pub looping: Arc<AtomicBool>,
    pub metadata: Arc<Mutex<InternalVideoMetadata>>,
    pub track_cache: Arc<Mutex<TrackCache>>,
    pub orientation: InternalVideoOrientationConfig,
    pub aspect: InternalAspectRatioMode,
    /// Frame source reused across shell rebuilds so the Flutter texture keeps
    /// receiving frames after a URI ↔ asset switch (appsink platforms).
    pub frame_sink: Arc<crate::playback::frame::FrameSink>,
    /// Caps-driven ImageReader resize + overlay sync (Android texture path).
    #[cfg(target_os = "android")]
    pub overlay_size_sync: Option<OverlaySizeSync>,
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
            frame_sink: self.frame_sink.clone(),
            #[cfg(target_os = "android")]
            overlay_size_sync: self.overlay_size_sync.clone(),
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
    if !shell.is_uri() {
        teardown_shell(shell);
        surface.mark_shell_rebuilt();
        *shell = install_uri_shell(
            &swap.emitter,
            &swap.looping,
            replay,
            Some(swap.metadata.clone()),
            Some(swap.track_cache.clone()),
            surface,
            &swap.frame_sink,
            #[cfg(target_os = "android")]
            swap.overlay_size_sync.clone(),
        )?;
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let overlay_sink = surface.overlay_sink_slot().cloned();
            wire_overlay_sync(shell, surface.stored_handle(), overlay_sink);
            if let Some(slot) = surface.overlay_sink_slot() {
                shell.sync_overlay_sink_slot(slot);
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        wire_overlay_sync(shell, surface.stored_handle());
    }
    surface.rebind_cached_overlay(shell)?;
    shell.apply_aspect_ratio(swap.aspect);
    shell.apply_orientation(swap.orientation)?;
    pipeline_set_uri(shell, uri, replay, surface)
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
        &swap.frame_sink,
        #[cfg(target_os = "android")]
        swap.overlay_size_sync.clone(),
    )?;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let overlay_sink = surface.overlay_sink_slot().cloned();
        wire_overlay_sync(shell, surface.stored_handle(), overlay_sink);
        if let Some(slot) = surface.overlay_sink_slot() {
            shell.sync_overlay_sink_slot(slot);
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    wire_overlay_sync(shell, surface.stored_handle());
    surface.rebind_cached_overlay(shell)?;
    shell.apply_aspect_ratio(swap.aspect);
    replay.at_eos.store(false, Ordering::SeqCst);
    preroll_asset_shell(
        shell,
        surface,
        "gst: deferring asset Paused preroll until Android overlay is bound",
    )
}

fn preroll_asset_shell(
    shell: &PipelineShell,
    surface: &VideoSurface,
    defer_log: &str,
) -> Result<()> {
    surface
        .overlay_session()
        .apply_load_preroll(shell, surface, defer_log)
}

fn pipeline_set_uri(
    shell: &PipelineShell,
    uri: &str,
    replay: &PlayReplayContext,
    surface: &VideoSurface,
) -> Result<()> {
    replay.at_eos.store(false, Ordering::SeqCst);
    shell.set_uri(uri)?;
    surface.overlay_session().apply_load_preroll(
        shell,
        surface,
        "gst: deferring URI Paused preroll until Android overlay is bound",
    )
}
