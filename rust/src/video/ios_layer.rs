use std::ffi::CStr;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;

#[cfg(target_os = "ios")]
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

#[cfg(target_os = "ios")]
use crate::platform_view_ios::{attach_layer_on_main_thread_sync, host_view_ready_for_attach};
use crate::playback::shell::PipelineShell;
use crate::playback::state::set_state_sync;

/// Guards attach / rollback against dispose or media reload.
#[cfg(target_os = "ios")]
#[derive(Clone)]
pub struct IosAttachLifecycle {
    running: Arc<AtomicBool>,
    work_generation: u64,
    overlay_generation: Arc<AtomicU64>,
}

#[cfg(not(target_os = "ios"))]
#[derive(Clone, Copy)]
pub struct IosAttachLifecycle;

#[cfg(not(target_os = "ios"))]
impl IosAttachLifecycle {
    pub fn is_stale(&self) -> bool {
        false
    }
}

#[cfg(target_os = "ios")]
impl IosAttachLifecycle {
    pub fn new(
        running: Arc<AtomicBool>,
        work_generation: u64,
        overlay_generation: Arc<AtomicU64>,
    ) -> Self {
        Self {
            running,
            work_generation,
            overlay_generation,
        }
    }

    pub fn is_stale(&self) -> bool {
        !self.running.load(Ordering::SeqCst)
            || self.work_generation != self.overlay_generation.load(Ordering::SeqCst)
    }
}

/// True when the video sink pad has negotiated `video/*` caps.
pub fn video_sink_has_video_caps(sink: &gst::Element) -> bool {
    sink.static_pad("sink")
        .and_then(|pad| pad.current_caps())
        .and_then(|caps| caps.structure(0).map(|s| s.name().starts_with("video/")))
        .unwrap_or(false)
}

#[cfg(target_os = "ios")]
extern "C" {
    fn CFRetain(cf: *const std::ffi::c_void) -> *const std::ffi::c_void;
    fn CFRelease(cf: *const std::ffi::c_void);
}

/// Outcome of attempting to schedule an iOS CALayer attach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IosLayerAttachOutcome {
    /// CALayer attach completed synchronously on the main thread.
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
    if sink.find_property("layer").is_none() {
        return Err(anyhow!("sink does not have layer property"));
    }
    let layer_ptr = sink.property::<glib::Pointer>("layer");
    if layer_ptr.is_null() {
        return Err(anyhow!("sink layer not ready yet"));
    }
    #[cfg(target_os = "ios")]
    unsafe {
        let retained = CFRetain(layer_ptr as *const std::ffi::c_void);
        Ok(retained as usize)
    }
    #[cfg(not(target_os = "ios"))]
    {
        Ok(layer_ptr as usize)
    }
}

/// Prerolls `pipeline` to PAUSED when media is pending (iOS Tutorial 4 attach flow).
/// Call without holding `PipelineShell` mutex — bus handlers may re-enter attach.
pub fn preroll_pipeline_for_ios_layer(pipeline: &gst::Pipeline) -> Result<()> {
    let (_, mut current, mut pending) = pipeline.state(gst::ClockTime::ZERO);
    log::info!("gst: ios preroll enter current={current:?} pending={pending:?}");
    if current < gst::State::Ready {
        set_state_sync(pipeline, gst::State::Ready)?;
        (_, current, pending) = pipeline.state(gst::ClockTime::ZERO);
    }

    if current == gst::State::Ready && pending == gst::State::VoidPending {
        set_state_sync(pipeline, gst::State::Paused)?;
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
        if let Err(e) = set_state_sync(pipeline, gst::State::Ready) {
            log::warn!("gst: ios layer attach rollback to READY: {e:#}");
        } else {
            log::info!("gst: ios layer attach preroll rolled back to READY");
        }
    }
}

#[cfg(not(target_os = "ios"))]
fn rollback_pipeline_from_ios_preroll(_pipeline: &gst::Pipeline) {}

/// Prerolls the pipeline to PAUSED when media is pending.
pub fn preroll_for_ios_layer(shell: &PipelineShell) -> Result<()> {
    if !shell.has_pending_media() {
        return Ok(());
    }
    preroll_pipeline_for_ios_layer(&shell.pipeline)
}

/// Sync-attaches `layer` on the main thread; on success optionally prerolls when media is pending.
#[cfg(target_os = "ios")]
fn sync_attach_then_preroll(
    host_view: usize,
    layer: usize,
    pipeline: &gst::Pipeline,
    has_pending_media: bool,
    lifecycle: &IosAttachLifecycle,
) -> Result<bool> {
    if lifecycle.is_stale() {
        release_sink_layer(layer);
        return Ok(false);
    }
    if attach_layer_on_main_thread_sync(host_view, layer) {
        log::info!("gst: ios layer attached sync host={host_view:#x} layer={layer:#x}");
        if has_pending_media {
            if let Err(e) = preroll_pipeline_for_ios_layer(pipeline) {
                log::warn!("gst: ios preroll after sync attach failed: {e:#}");
                rollback_pipeline_from_ios_preroll(pipeline);
                return Ok(false);
            }
        }
        Ok(true)
    } else {
        release_sink_layer(layer);
        log::debug!(
            "gst: ios layer attach sync failed host={host_view:#x} (zero bounds or verify failed)"
        );
        Ok(false)
    }
}

/// Attaches the sink CALayer on the main thread (sync), then prerolls when media is pending.
///
/// Preferred order: read layer at READY → sync attach → PAUSED preroll (decode with layer in hierarchy).
/// Fallback when `layer` is unreadable at READY: preroll → read → immediate sync attach.
#[cfg(target_os = "ios")]
pub fn attach_ios_video_layer_with_completion<F>(
    pipeline: &gst::Pipeline,
    has_pending_media: bool,
    sink: &gst::Element,
    host_view: usize,
    lifecycle: IosAttachLifecycle,
    on_complete: F,
) -> Result<IosLayerAttachOutcome>
where
    F: FnOnce(bool) + Send + 'static,
{
    if host_view == 0 {
        return Ok(IosLayerAttachOutcome::Skipped);
    }

    if lifecycle.is_stale() {
        return Ok(IosLayerAttachOutcome::Skipped);
    }

    if !host_view_ready_for_attach(host_view) {
        log::debug!("gst: ios layer attach deferred host={host_view:#x} (zero bounds)");
        return Ok(IosLayerAttachOutcome::LayerNotReady);
    }

    // Primary path: layer readable at READY — attach before any pipeline preroll.
    if let Ok(layer) = read_sink_layer(sink) {
        log::info!("gst: ios read_sink_layer ok at READY layer={layer:#x}");
        let attached =
            sync_attach_then_preroll(host_view, layer, pipeline, has_pending_media, &lifecycle)?;
        on_complete(attached);
        return if attached {
            Ok(IosLayerAttachOutcome::Scheduled)
        } else {
            Ok(IosLayerAttachOutcome::LayerNotReady)
        };
    }

    if !has_pending_media {
        log::debug!("gst: ios read_sink_layer at READY: layer not ready (no pending media)");
        return Ok(IosLayerAttachOutcome::LayerNotReady);
    }

    // Fallback: preroll exposes `layer`, then sync attach immediately in the same Gst turn.
    log::info!("gst: ios layer not at READY — fallback preroll then sync attach");
    let prerolled = match preroll_pipeline_for_ios_layer(pipeline) {
        Ok(()) => true,
        Err(e) => {
            log::warn!("gst: ios fallback preroll failed: {e:#}");
            false
        }
    };

    if lifecycle.is_stale() {
        if prerolled {
            rollback_pipeline_from_ios_preroll(pipeline);
        }
        return Ok(IosLayerAttachOutcome::Skipped);
    }

    match read_sink_layer(sink) {
        Ok(layer) => {
            log::info!("gst: ios read_sink_layer ok after fallback preroll layer={layer:#x}");
            let attached = sync_attach_then_preroll(host_view, layer, pipeline, false, &lifecycle)?;
            if !attached && prerolled {
                rollback_pipeline_from_ios_preroll(pipeline);
            }
            on_complete(attached);
            Ok(if attached {
                IosLayerAttachOutcome::Scheduled
            } else {
                IosLayerAttachOutcome::LayerNotReady
            })
        }
        Err(e) => {
            if prerolled && !lifecycle.is_stale() {
                rollback_pipeline_from_ios_preroll(pipeline);
            }
            log::debug!("gst: ios read_sink_layer after fallback preroll: {e:#}");
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
    _lifecycle: IosAttachLifecycle,
    _on_complete: F,
) -> Result<IosLayerAttachOutcome>
where
    F: FnOnce(bool) + Send + 'static,
{
    Ok(IosLayerAttachOutcome::Skipped)
}

#[cfg(target_os = "ios")]
pub fn setup_ios_notify_layer_handler(
    sink: &gst::Element,
    stored: Arc<parking_lot::Mutex<Option<usize>>>,
    overlay_bound: Arc<AtomicBool>,
) {
    if sink.find_property("layer").is_none() {
        log::info!("gst: sink is not avsamplebufferlayersink (no layer property), skipping notify/probe setup");
        return;
    }

    if let Some(pad) = sink.static_pad("sink") {
        let overlay_bound_clone = overlay_bound.clone();
        pad.add_probe(gst::PadProbeType::BUFFER, move |_, _| {
            if !overlay_bound_clone.load(Ordering::SeqCst) {
                log::info!("gst: pad probe - blocking buffer until layer is attached");
                while !overlay_bound_clone.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                log::info!("gst: pad probe - layer is attached, releasing buffer");
            }
            gst::PadProbeReturn::Ok
        });
    }

    let stored_clone = stored.clone();
    let overlay_bound_clone = overlay_bound.clone();
    sink.connect_notify(Some("layer"), move |sink, _| {
        log::info!("gst: notify::layer signal received");
        if let Ok(layer) = read_sink_layer(sink) {
            log::info!("gst: notify::layer read layer ok: {layer:#x}");
            let host_view = match *stored_clone.lock() {
                Some(h) if h != 0 => h,
                _ => {
                    release_sink_layer(layer);
                    return;
                }
            };
            if !host_view_ready_for_attach(host_view) {
                log::debug!("gst: notify::layer - host view not ready");
                release_sink_layer(layer);
                return;
            }
            if attach_layer_on_main_thread_sync(host_view, layer) {
                log::info!(
                    "gst: notify::layer - layer attached sync host={host_view:#x} layer={layer:#x}"
                );
                overlay_bound_clone.store(true, Ordering::SeqCst);
            } else {
                release_sink_layer(layer);
            }
        }
    });
}

#[cfg(not(target_os = "ios"))]
pub fn setup_ios_notify_layer_handler(
    _sink: &gst::Element,
    _stored: std::sync::Arc<parking_lot::Mutex<Option<usize>>>,
    _overlay_bound: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
}
