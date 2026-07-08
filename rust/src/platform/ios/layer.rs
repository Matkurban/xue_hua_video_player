use std::ffi::CStr;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;

#[cfg(target_os = "ios")]
use crate::platform_view_ios::{attach_layer_on_main_thread_async, host_view_ready_for_attach};
use crate::playback::shell::set_element_state_sync;
use crate::playback::shell::PipelineShell;

#[cfg(target_os = "ios")]
extern "C" {
    fn CFRetain(cf: *const std::ffi::c_void) -> *const std::ffi::c_void;
    fn CFRelease(cf: *const std::ffi::c_void);
}

/// Outcome of attempting to schedule an iOS CALayer attach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IosLayerAttachOutcome {
    /// CALayer attach completed (sync path) or was scheduled async for re-layout.
    Scheduled,
    /// Sink `layer` property not ready yet; retry on bus `READY→PAUSED` / `AsyncDone` or layout.
    LayerNotReady,
    /// No host view to attach to.
    Skipped,
}

/// Releases +1 retain from [`read_sink_layer`] when attach is skipped or deferred.
#[cfg(target_os = "ios")]
pub fn release_sink_layer(layer: usize) {
    if layer != 0 {
        unsafe {
            CFRelease(layer as *const std::ffi::c_void);
        }
    }
}

#[cfg(not(target_os = "ios"))]
pub fn release_sink_layer(_layer: usize) {}

/// Reads the `layer` property from `avsamplebufferlayersink` (CALayer pointer).
/// On iOS the returned pointer is +1 retained; release via attach shim or `release_sink_layer`.
pub fn read_sink_layer(sink: &gst::Element) -> Result<usize> {
    unsafe {
        let mut layer: glib::ffi::gpointer = std::ptr::null_mut();
        glib::gobject_ffi::g_object_get(
            sink.as_ptr() as *mut glib::gobject_ffi::GObject,
            CStr::from_bytes_with_nul(b"layer\0")
                .map_err(|_| anyhow!("layer property name"))?
                .as_ptr(),
            &mut layer,
            std::ptr::null_mut::<glib::ffi::gpointer>(),
        );
        if layer.is_null() {
            return Err(anyhow!("sink layer not ready yet"));
        }
        #[cfg(target_os = "ios")]
        {
            let retained = CFRetain(layer as *const std::ffi::c_void);
            Ok(retained as usize)
        }
        #[cfg(not(target_os = "ios"))]
        {
            Ok(layer as usize)
        }
    }
}

/// Prerolls `pipeline` to PAUSED when media is pending (iOS Tutorial 4 attach flow).
/// Call without holding `PipelineShell` mutex — bus handlers may re-enter attach.
pub fn preroll_pipeline_for_ios_layer(pipeline: &gst::Pipeline) -> Result<()> {
    let (_, mut current, mut pending) = pipeline.state(gst::ClockTime::ZERO);
    log::info!("gst: ios preroll enter current={current:?} pending={pending:?}");
    if current < gst::State::Ready {
        set_element_state_sync(pipeline, gst::State::Ready)?;
        (_, current, pending) = pipeline.state(gst::ClockTime::ZERO);
    }

    if current == gst::State::Ready && pending == gst::State::VoidPending {
        set_element_state_sync(pipeline, gst::State::Paused)?;
        let (_, after, _) = pipeline.state(gst::ClockTime::ZERO);
        log::info!("gst: ios preroll reached {after:?}");
    }
    Ok(())
}

/// Reverts a PAUSED preroll when CALayer attach cannot complete (stops decode without layer).
#[cfg(target_os = "ios")]
fn rollback_pipeline_from_ios_preroll(pipeline: &gst::Pipeline) {
    let (_, current, _) = pipeline.state(gst::ClockTime::ZERO);
    if current >= gst::State::Paused {
        if let Err(e) = set_element_state_sync(pipeline, gst::State::Ready) {
            log::warn!("gst: ios layer attach rollback to READY: {e:#}");
        } else {
            log::info!("gst: ios layer attach preroll rolled back to READY");
        }
    }
}

#[cfg(not(target_os = "ios"))]
fn rollback_pipeline_from_ios_preroll(_pipeline: &gst::Pipeline) {}

/// Prerolls the pipeline to PAUSED when media is pending.
#[cfg(target_os = "ios")]
pub fn preroll_for_ios_layer(shell: &PipelineShell) -> Result<()> {
    shell.preroll_for_ios_layer()
}

#[cfg(not(target_os = "ios"))]
pub fn preroll_for_ios_layer(_shell: &PipelineShell) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "ios")]
fn schedule_layer_attach_async<F>(
    host_view: usize,
    layer: usize,
    pipeline: gst::Pipeline,
    prerolled: bool,
    on_complete: F,
) -> IosLayerAttachOutcome
where
    F: FnOnce(bool) + Send + 'static,
{
    attach_layer_on_main_thread_async(host_view, layer, move |attached| {
        if !attached {
            release_sink_layer(layer);
            if prerolled {
                rollback_pipeline_from_ios_preroll(&pipeline);
            }
            log::debug!(
                "gst: ios layer attach async deferred host={host_view:#x} (zero bounds or verify failed)"
            );
        } else {
            log::info!("gst: ios layer attached async host={host_view:#x} layer={layer:#x}");
        }
        on_complete(attached);
    });
    IosLayerAttachOutcome::Scheduled
}

/// Prerolls when needed, async-attaches the sink CALayer on the main thread, then reports success.
#[cfg(target_os = "ios")]
pub fn attach_ios_video_layer_with_completion<F>(
    pipeline: &gst::Pipeline,
    has_pending_media: bool,
    sink: &gst::Element,
    host_view: usize,
    on_complete: F,
) -> Result<IosLayerAttachOutcome>
where
    F: FnOnce(bool) + Send + 'static,
{
    if host_view == 0 {
        return Ok(IosLayerAttachOutcome::Skipped);
    }

    if !host_view_ready_for_attach(host_view) {
        log::debug!("gst: ios layer attach deferred host={host_view:#x} (zero bounds)");
        return Ok(IosLayerAttachOutcome::LayerNotReady);
    }

    let pipeline_owned = pipeline.clone();

    if !has_pending_media {
        return match read_sink_layer(sink) {
            Ok(layer) => Ok(schedule_layer_attach_async(
                host_view,
                layer,
                pipeline_owned,
                false,
                on_complete,
            )),
            Err(e) => Err(anyhow!("sink layer not ready yet (no pending media): {e}")),
        };
    }

    preroll_pipeline_for_ios_layer(pipeline)?;
    let prerolled = true;

    match read_sink_layer(sink) {
        Ok(layer) => {
            log::info!("gst: ios read_sink_layer ok after preroll layer={layer:#x}");
            Ok(schedule_layer_attach_async(
                host_view,
                layer,
                pipeline_owned,
                prerolled,
                on_complete,
            ))
        }
        Err(e) => {
            rollback_pipeline_from_ios_preroll(pipeline);
            log::debug!("gst: ios read_sink_layer after preroll: {e:#}");
            Ok(IosLayerAttachOutcome::LayerNotReady)
        }
    }
}

#[cfg(not(target_os = "ios"))]
pub fn attach_ios_video_layer_with_completion<F>(
    _pipeline: &gst::Pipeline,
    _has_pending_media: bool,
    _sink: &gst::Element,
    _host_view: usize,
    _on_complete: F,
) -> Result<IosLayerAttachOutcome>
where
    F: FnOnce(bool) + Send + 'static,
{
    Ok(IosLayerAttachOutcome::Skipped)
}
