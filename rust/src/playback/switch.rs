use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::media::ResolvedSource;
use crate::playback::tracks::TrackCache;
use crate::playback::bus::Emitter;
use crate::playback::shell::{
    install_asset_shell, install_uri_shell, teardown_shell, wire_overlay_sync, PipelineShell,
    SourceKind,
};
use crate::playback::state::set_state_sync;
use crate::playback::surface::{assign_overlay_sink, VideoSurface};
use crate::video::{
    info::InternalVideoMetadata,
    orientation::{InternalAspectRatioMode, InternalVideoOrientationConfig},
};
use crate::video::orientation::apply_orientation_to_playbin;

/// Shared state required when swapping the active [`PipelineShell`].
pub struct SwitchContext {
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

/// Rebuilds or reconfigures the pipeline shell for `resolved` and applies overlay/orientation.
pub fn switch_shell(
    shell: &mut PipelineShell,
    resolved: ResolvedSource,
    ctx: &SwitchContext,
) -> Result<()> {
    match resolved {
        ResolvedSource::Uri(uri) => switch_uri_shell(shell, &uri, ctx),
        ResolvedSource::AppSrc(asset_key) => switch_asset_shell(shell, &asset_key, ctx),
    }
}

fn switch_uri_shell(shell: &mut PipelineShell, uri: &str, ctx: &SwitchContext) -> Result<()> {
    if shell.kind != SourceKind::Uri {
        teardown_shell(shell);
        *shell = install_uri_shell(
            &ctx.emitter,
            &ctx.looping,
            &ctx.desired_playing,
            &ctx.at_eos,
            &ctx.running,
            Some(ctx.metadata.clone()),
            Some(ctx.track_cache.clone()),
        )?;
        wire_overlay_sync(shell, ctx.surface.stored_handle());
        #[cfg(target_os = "macos")]
        if let Some(slot) = ctx.surface.macos_overlay_sink() {
            assign_overlay_sink(slot, &shell.video_sink);
        }
    }
    ctx.surface.rebind_cached_overlay(shell)?;
    ctx.aspect.apply_to_sink(&shell.video_sink);
    apply_orientation_to_playbin(shell.pipeline.upcast_ref::<gst::Element>(), ctx.orientation)?;
    let has_overlay = ctx.surface.has_cached_handle();
    pipeline_set_uri(&shell.pipeline, uri, &ctx.at_eos, has_overlay)
}

fn switch_asset_shell(shell: &mut PipelineShell, asset_key: &str, ctx: &SwitchContext) -> Result<()> {
    teardown_shell(shell);
    *shell = install_asset_shell(
        asset_key,
        &ctx.emitter,
        &ctx.looping,
        &ctx.desired_playing,
        &ctx.at_eos,
        &ctx.running,
        Some(ctx.metadata.clone()),
    )?;
    wire_overlay_sync(shell, ctx.surface.stored_handle());
    #[cfg(target_os = "macos")]
    if let Some(slot) = ctx.surface.macos_overlay_sink() {
        assign_overlay_sink(slot, &shell.video_sink);
    }
    ctx.surface.rebind_cached_overlay(shell)?;
    ctx.aspect.apply_to_sink(&shell.video_sink);
    ctx.at_eos.store(false, Ordering::SeqCst);
    preroll_asset_shell(shell, ctx.surface.has_cached_handle())
}

fn preroll_asset_shell(shell: &PipelineShell, has_overlay: bool) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        if has_overlay {
            set_state_sync(&shell.pipeline, gst::State::Paused)?;
        } else {
            crate::diag::logcat_info(
                "gst: deferring asset Paused preroll until Android overlay is bound",
            );
        }
        return Ok(());
    }
    #[cfg(not(target_os = "android"))]
    set_state_sync(&shell.pipeline, gst::State::Paused)
}

fn pipeline_set_uri(
    pipeline: &gst::Pipeline,
    uri: &str,
    at_eos: &AtomicBool,
    has_overlay: bool,
) -> Result<()> {
    at_eos.store(false, Ordering::SeqCst);
    set_state_sync(pipeline, gst::State::Ready)?;
    pipeline.set_property("uri", uri);
    if has_overlay {
        set_state_sync(pipeline, gst::State::Paused)
    } else {
        #[cfg(target_os = "android")]
        crate::diag::logcat_info(
            "gst: deferring URI Paused preroll until Android overlay is bound",
        );
        #[cfg(not(target_os = "android"))]
        set_state_sync(pipeline, gst::State::Paused)?;
        Ok(())
    }
}
