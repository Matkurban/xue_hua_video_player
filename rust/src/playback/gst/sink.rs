use std::sync::Arc;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video::{
    self as gst_video,
    prelude::{VideoOverlayExt, VideoOverlayExtManual},
};
use parking_lot::Mutex;

/// GStreamer-recommended video sink element name per platform.
pub fn video_sink_factory_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "d3d11videosink"
    }
    #[cfg(target_os = "macos")]
    {
        "osxvideosink"
    }
    #[cfg(target_os = "ios")]
    {
        "avsamplebufferlayersink"
    }
    #[cfg(all(
        not(target_os = "windows"),
        not(target_os = "macos"),
        not(target_os = "ios")
    ))]
    {
        "glimagesink"
    }
}

fn configure_video_sink(element: &gst::Element) {
    if element.find_property("force-aspect-ratio").is_some() {
        element.set_property("force-aspect-ratio", true);
    }
}

/// Creates the platform-recommended video sink (`glimagesink` or `d3d11videosink`).
pub fn create_platform_video_sink() -> Result<gst::Element> {
    let name = video_sink_factory_name();
    let sink = gst::ElementFactory::make(name)
        .build()
        .map_err(|e| anyhow!("failed to create {name}: {e}"))?;
    configure_video_sink(&sink);
    Ok(sink)
}

/// Binds a native window/surface handle to a VideoOverlay-capable sink.
///
/// Does not call `expose` — callers should expose only after the first video frame
/// or when the render rectangle is known (avoids green/clear framebuffer flash).
pub fn set_overlay_window_handle(video_sink: &gst::Element, handle: usize) -> Result<()> {
    let overlay = video_sink
        .clone()
        .dynamic_cast::<gst_video::VideoOverlay>()
        .map_err(|_| anyhow!("video sink does not implement VideoOverlay"))?;
    unsafe {
        overlay.set_window_handle(handle);
    }
    Ok(())
}

fn bind_overlay_element(overlay: &gst_video::VideoOverlay, handle: usize) {
    unsafe {
        overlay.set_window_handle(handle);
    }
}

/// Clears the overlay window handle (surface destroyed).
pub fn clear_overlay_window_handle(video_sink: &gst::Element) -> Result<()> {
    set_overlay_window_handle(video_sink, 0)
}

/// Requests a redraw after surface geometry changes.
///
/// GStreamer Android tutorial 3 calls `gst_video_overlay_expose` twice because
/// of how surface size changes propagate through the OpenGL ES pipeline.
pub fn expose_overlay(video_sink: &gst::Element) {
    if let Ok(overlay) = video_sink.clone().dynamic_cast::<gst_video::VideoOverlay>() {
        overlay.expose();
        overlay.expose();
    }
}

/// Sets the embedded view rectangle and requests a redraw.
///
/// Do not call on iOS from the Gst thread — `glimagesink` layer updates must not
/// race UIKit; iOS sizing is handled by `EaglUIView` layout on the main thread.
pub fn set_overlay_render_rectangle(video_sink: &gst::Element, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    if let Ok(overlay) = video_sink.clone().dynamic_cast::<gst_video::VideoOverlay>() {
        let _ = overlay.set_render_rectangle(0, 0, width, height);
        overlay.expose();
        overlay.expose();
    }
}

/// Handles `prepare-window-handle` on the pipeline bus sync handler.
pub fn bus_sync_reply_for_overlay_message(
    msg: &gst::MessageRef,
    cached_handle: Option<usize>,
) -> gst::BusSyncReply {
    use gstreamer_video::is_video_overlay_prepare_window_handle_message;

    if !is_video_overlay_prepare_window_handle_message(msg) {
        return gst::BusSyncReply::Pass;
    }
    let Some(handle) = cached_handle else {
        log::warn!("prepare-window-handle received but no overlay handle is cached yet");
        return gst::BusSyncReply::Pass;
    };
    #[cfg(target_os = "ios")]
    {
        return bus_sync_reply_for_ios_overlay(msg, Some(handle), None);
    }
    #[cfg(target_os = "macos")]
    {
        return bus_sync_reply_for_macos_overlay(msg);
    }
    #[cfg(all(not(target_os = "ios"), not(target_os = "macos")))]
    if let Some(src) = msg.src() {
        if let Ok(overlay) = src.clone().dynamic_cast::<gst_video::VideoOverlay>() {
            log::info!("prepare-window-handle: binding overlay handle {handle:#x}");
            bind_overlay_element(&overlay, handle);
        }
    }
    #[cfg(all(not(target_os = "ios"), not(target_os = "macos")))]
    gst::BusSyncReply::Drop
}

/// macOS `prepare-window-handle`: pass — `osxvideosink` must bind on the main thread via
/// `MacosOverlayBackend::apply_gstreamer`, not from the Gst bus sync handler.
#[cfg(target_os = "macos")]
pub fn bus_sync_reply_for_macos_overlay(msg: &gst::MessageRef) -> gst::BusSyncReply {
    use gstreamer_video::is_video_overlay_prepare_window_handle_message;

    if !is_video_overlay_prepare_window_handle_message(msg) {
        return gst::BusSyncReply::Pass;
    }
    log::debug!(
        "prepare-window-handle: ignored on macOS (main-thread MacosOverlayBackend applies bind)"
    );
    gst::BusSyncReply::Pass
}

/// iOS `prepare-window-handle`: pass — `avsamplebufferlayersink` uses [`IosOverlaySession`] async CALayer attach, not VideoOverlay sync bind.
#[cfg(target_os = "ios")]
pub fn bus_sync_reply_for_ios_overlay(
    msg: &gst::MessageRef,
    _cached_handle: Option<usize>,
    _overlay_sink: Option<&Arc<Mutex<gst::Element>>>,
) -> gst::BusSyncReply {
    use gstreamer_video::is_video_overlay_prepare_window_handle_message;

    if !is_video_overlay_prepare_window_handle_message(msg) {
        return gst::BusSyncReply::Pass;
    }
    log::debug!("prepare-window-handle: ignored on iOS (IosOverlaySession handles CALayer attach)");
    gst::BusSyncReply::Pass
}

/// Installs a bus sync handler that answers `prepare-window-handle` for VideoOverlay sinks.
pub fn attach_overlay_bus_sync_handler(
    pipeline: &gst::Pipeline,
    overlay_handle: Arc<Mutex<Option<usize>>>,
    #[cfg(any(target_os = "macos", target_os = "ios"))] overlay_sink: Option<
        Arc<Mutex<gst::Element>>,
    >,
) {
    let bus = match pipeline.bus() {
        Some(bus) => bus,
        None => return,
    };
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let overlay_sink_bus = overlay_sink.clone();
    bus.set_sync_handler(move |_bus, msg| {
        #[cfg(target_os = "ios")]
        {
            let handle = *overlay_handle.lock();
            return bus_sync_reply_for_ios_overlay(msg, handle, overlay_sink_bus.as_ref());
        }
        #[cfg(target_os = "macos")]
        {
            return bus_sync_reply_for_macos_overlay(msg);
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        {
            let handle = *overlay_handle.lock();
            bus_sync_reply_for_overlay_message(msg, handle)
        }
    });
}
