use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::media::ResolvedSource;
use crate::playback::bus::Emitter;
use crate::playback::shell::{
    install_asset_shell, install_uri_shell, teardown_shell, wire_overlay_sync, PipelineShell,
    SourceKind,
};
use crate::playback::state::set_state_sync;
#[cfg(target_os = "macos")]
use crate::playback::surface::assign_overlay_sink;
#[cfg(target_os = "android")]
use crate::playback::surface::refresh_android_overlay_on_gst;
use crate::playback::surface::VideoSurface;
use crate::playback::tracks::TrackCache;
use crate::video::orientation::apply_orientation_to_playbin;
use crate::video::{
    info::InternalVideoMetadata,
    orientation::{InternalAspectRatioMode, InternalVideoOrientationConfig},
};

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
        ctx.surface.mark_shell_rebuilt();
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
    pipeline_set_uri(shell, uri, &ctx.at_eos, has_overlay, &ctx.surface)
}

fn switch_asset_shell(
    shell: &mut PipelineShell,
    asset_key: &str,
    ctx: &SwitchContext,
) -> Result<()> {
    teardown_shell(shell);
    ctx.surface.mark_shell_rebuilt();
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
    preroll_asset_shell(shell, ctx.surface.has_cached_handle(), &ctx.surface)
}

/// Replays an asset from EOS by tearing down and rebuilding the shell (fresh decodebin).
pub fn replay_asset_shell(shell: &mut PipelineShell, ctx: &SwitchContext) -> Result<()> {
    let key = shell
        .asset_key
        .clone()
        .ok_or_else(|| anyhow!("asset replay requested but asset_key missing"))?;
    switch_asset_shell(shell, &key, ctx)?;
    set_state_sync(&shell.pipeline, gst::State::Playing)?;
    #[cfg(target_os = "android")]
    crate::diag::logcat_info("gst: AppSrc replay from EOS (shell reload)");
    Ok(())
}

fn preroll_asset_shell(
    shell: &PipelineShell,
    has_overlay: bool,
    surface: &VideoSurface,
) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        if has_overlay {
            set_state_sync(&shell.pipeline, gst::State::Paused)?;
            if let Some(handle) = *surface.stored_handle().lock() {
                let (width, height) = surface.cached_dimensions();
                refresh_android_overlay_on_gst(
                    shell,
                    handle,
                    width,
                    height,
                    "after Paused preroll",
                )?;
            }
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
    shell: &PipelineShell,
    uri: &str,
    at_eos: &AtomicBool,
    has_overlay: bool,
    surface: &VideoSurface,
) -> Result<()> {
    let pipeline = &shell.pipeline;
    at_eos.store(false, Ordering::SeqCst);
    set_state_sync(pipeline, gst::State::Ready)?;
    pipeline.set_property("uri", uri);
    if has_overlay {
        set_state_sync(pipeline, gst::State::Paused)?;
        #[cfg(target_os = "android")]
        if let Some(handle) = *surface.stored_handle().lock() {
            let (width, height) = surface.cached_dimensions();
            refresh_android_overlay_on_gst(shell, handle, width, height, "after Paused preroll")?;
        }
        Ok(())
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
