use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;

use crate::platform_view_ios::attach_layer_on_main_thread;
use crate::playback::shell::PipelineShell;
use crate::playback::state::set_state_sync;

/// Reads the `layer` property from `avsamplebufferlayersink` (CALayer pointer).
pub fn read_sink_layer(sink: &gst::Element) -> Result<usize> {
    let layer: glib::ffi::gpointer = sink.property("layer");
    if layer.is_null() {
        return Err(anyhow!("sink layer not ready yet"));
    }
    Ok(layer as usize)
}

/// Ensures pipeline preroll far enough to expose the sink layer, then attaches on main thread.
pub fn attach_ios_video_layer(
    shell: &PipelineShell,
    sink: &gst::Element,
    host_view: usize,
) -> Result<()> {
    if host_view == 0 {
        return Ok(());
    }

    if let Ok(layer) = read_sink_layer(sink) {
        attach_layer_on_main_thread(host_view, layer);
        log::info!("gst: ios layer attached host={host_view:#x} layer={layer:#x}");
        return Ok(());
    }

    let (_, current, _) = shell.pipeline.state(gst::ClockTime::ZERO);
    if current < gst::State::Ready {
        set_state_sync(&shell.pipeline, gst::State::Ready)?;
    }
    if let Ok(layer) = read_sink_layer(sink) {
        attach_layer_on_main_thread(host_view, layer);
        log::info!("gst: ios layer attached after Ready host={host_view:#x}");
        return Ok(());
    }

    if shell.has_pending_media() && current == gst::State::Ready {
        set_state_sync(&shell.pipeline, gst::State::Paused)?;
    }
    let layer = read_sink_layer(sink)?;
    attach_layer_on_main_thread(host_view, layer);
    log::info!("gst: ios layer attached after Paused preroll host={host_view:#x}");
    Ok(())
}
