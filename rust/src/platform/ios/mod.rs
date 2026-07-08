//! iOS main-thread UIKit / Core Animation helpers (CALayer attach, VideoOverlay bind).

use std::cell::RefCell;

use anyhow::Result;
use gstreamer as gst;

use crate::gst::spawn_on_gst_thread;
use crate::playback::gst::{clear_overlay_window_handle, set_overlay_window_handle};

pub mod layer;

pub use layer::{
    attach_ios_video_layer_with_completion, preroll_for_ios_layer, preroll_pipeline_for_ios_layer,
    read_sink_layer, release_sink_layer, IosLayerAttachOutcome,
};

extern "C" {
    fn xhvp_dispatch_sync_main_fn(
        fun: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
        ctx: *mut std::ffi::c_void,
    );
    fn xhvp_ios_host_view_has_bounds(host_view: usize) -> bool;
    fn xhvp_ios_attach_layer_to_host_async(
        host_view: usize,
        layer: usize,
        complete_fn: Option<unsafe extern "C" fn(bool, *mut std::ffi::c_void)>,
        complete_ctx: *mut std::ffi::c_void,
    );
    fn xhvp_ios_attach_layer_to_host_sync(host_view: usize, layer: usize) -> bool;
    fn xhvp_ios_release_layer_main(layer: usize);
    fn xhvp_ios_detach_sink_layers(host_view: usize);
    fn xhvp_ios_layer_status(layer: usize, out_status: *mut i32, out_error_code: *mut i32);
}

/// Releases a +1 retained sink layer on the main thread (dealloc must be main).
pub fn release_layer_on_main_thread(layer: usize) {
    if layer != 0 {
        unsafe { xhvp_ios_release_layer_main(layer) }
    }
}

/// Removes the sink CALayer(s) from the host view on the main thread.
pub fn detach_sink_layers_on_main_thread(host_view: usize) {
    if host_view != 0 {
        unsafe { xhvp_ios_detach_sink_layers(host_view) }
    }
}

/// Reads the `AVSampleBufferDisplayLayer` status (0 unknown, 1 rendering, 2
/// failed, -1 not a display layer) and error code for on-device diagnostics.
pub fn layer_status(layer: usize) -> (i32, i32) {
    let mut status: i32 = -1;
    let mut error_code: i32 = 0;
    unsafe { xhvp_ios_layer_status(layer, &mut status, &mut error_code) }
    (status, error_code)
}

thread_local! {
    static MAIN_THREAD_BIND: RefCell<Option<(gst::Element, usize)>> = const { RefCell::new(None) };
}

struct LayerAttachComplete(Box<dyn FnOnce(bool) + Send>);

unsafe extern "C" fn trampoline_bind_overlay(_ctx: *mut std::ffi::c_void) {
    let _ = _ctx;
    MAIN_THREAD_BIND.with(|cell| {
        if let Some((sink, handle)) = cell.borrow_mut().take() {
            let result = if handle == 0 {
                clear_overlay_window_handle(&sink)
            } else {
                set_overlay_window_handle(&sink, handle)
            };
            if let Err(e) = result {
                log::warn!("ios main-thread overlay bind: {e:#}");
            }
        }
    });
}

/// Runs `set_window_handle` on the UIKit main thread (bus sync / glimagesink fallback).
pub fn bind_overlay_on_main_thread(sink: &gst::Element, handle: usize) -> Result<()> {
    MAIN_THREAD_BIND.with(|cell| {
        *cell.borrow_mut() = Some((sink.clone(), handle));
    });
    unsafe {
        xhvp_dispatch_sync_main_fn(Some(trampoline_bind_overlay), std::ptr::null_mut());
    }
    Ok(())
}

unsafe extern "C" fn trampoline_layer_attach_complete(attach_ok: bool, ctx: *mut std::ffi::c_void) {
    if ctx.is_null() {
        return;
    }
    let LayerAttachComplete(callback) = *Box::from_raw(ctx as *mut LayerAttachComplete);
    spawn_on_gst_thread(move || callback(attach_ok));
}

/// True when the Flutter host `UIView` has non-zero layout bounds.
pub fn host_view_ready_for_attach(host_view: usize) -> bool {
    if host_view == 0 {
        return false;
    }
    unsafe { xhvp_ios_host_view_has_bounds(host_view) }
}

/// Adds the sink's `CALayer` under the Flutter host view on the main thread synchronously
/// (resize-only; first attach uses [`attach_layer_on_main_thread_async`]).
/// Returns false when host bounds are zero or sublayer verification fails.
pub fn attach_layer_on_main_thread_sync(host_view: usize, layer: usize) -> bool {
    if host_view == 0 || layer == 0 {
        return false;
    }
    unsafe { xhvp_ios_attach_layer_to_host_sync(host_view, layer) }
}

/// Adds the sink's `CALayer` under the Flutter host view on the main thread, then runs
/// `on_complete(attach_ok)` back on the Gst thread. Never blocks xhvp-gst on the UI thread.
pub fn attach_layer_on_main_thread_async<F>(host_view: usize, layer: usize, on_complete: F)
where
    F: FnOnce(bool) + Send + 'static,
{
    if host_view == 0 || layer == 0 {
        spawn_on_gst_thread(move || on_complete(false));
        return;
    }
    let ctx = Box::into_raw(Box::new(LayerAttachComplete(Box::new(on_complete))));
    unsafe {
        xhvp_ios_attach_layer_to_host_async(
            host_view,
            layer,
            Some(trampoline_layer_attach_complete),
            ctx as *mut std::ffi::c_void,
        );
    }
}
