//! iOS main-thread UIKit / Core Animation helpers (CALayer attach, VideoOverlay bind).

use std::cell::RefCell;

use anyhow::Result;
use gstreamer as gst;

use crate::gst_runtime::spawn_on_gst_thread;
use crate::video::ios_layer::IosAttachLifecycle;
use crate::video::{clear_overlay_window_handle, set_overlay_window_handle};

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
    fn xhvp_ios_detach_sink_layers_from_host(host_view: usize);
}

thread_local! {
    static MAIN_THREAD_BIND: RefCell<Option<(gst::Element, usize, i32, i32)>> = const { RefCell::new(None) };
}

struct LayerAttachComplete {
    lifecycle: IosAttachLifecycle,
    callback: Box<dyn FnOnce(bool) + Send>,
}

unsafe extern "C" fn trampoline_bind_overlay(_ctx: *mut std::ffi::c_void) {
    let _ = _ctx;
    MAIN_THREAD_BIND.with(|cell| {
        if let Some((sink, handle, width, height)) = cell.borrow_mut().take() {
            let result = if handle == 0 {
                clear_overlay_window_handle(&sink)
            } else {
                set_overlay_window_handle(&sink, handle)
            };
            if let Err(e) = result {
                log::warn!("ios main-thread overlay bind: {e:#}");
            }
            if handle != 0 && width > 0 && height > 0 {
                crate::video::set_overlay_render_rectangle(&sink, width, height);
            }
        }
    });
}

/// Runs `set_window_handle` on the UIKit main thread (bus sync / glimagesink fallback).
pub fn bind_overlay_on_main_thread(sink: &gst::Element, handle: usize, width: i32, height: i32) -> Result<()> {
    MAIN_THREAD_BIND.with(|cell| {
        *cell.borrow_mut() = Some((sink.clone(), handle, width, height));
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
    let LayerAttachComplete { lifecycle, callback } =
        *Box::from_raw(ctx as *mut LayerAttachComplete);
    if lifecycle.is_stale() {
        log::debug!("gst: ios layer attach trampoline skipped (stale lifecycle)");
        return;
    }
    spawn_on_gst_thread(move || {
        if lifecycle.is_stale() {
            log::debug!("gst: ios layer attach callback skipped (stale lifecycle)");
            return;
        }
        callback(attach_ok);
    });
}

/// True when the Flutter host `UIView` has non-zero layout bounds.
pub fn host_view_ready_for_attach(host_view: usize) -> bool {
    if host_view == 0 {
        return false;
    }
    unsafe { xhvp_ios_host_view_has_bounds(host_view) }
}

/// Removes `AVSampleBufferDisplayLayer` sublayers from the host view (media reload).
pub fn detach_sink_layers_from_host(host_view: usize) {
    if host_view == 0 {
        return;
    }
    unsafe {
        xhvp_ios_detach_sink_layers_from_host(host_view);
    }
}

/// Adds the sink's `CALayer` under the Flutter host view on the main thread synchronously.
/// First bind and resize re-attach use this path from xhvp-gst (`dispatch_sync` main queue).
/// Returns false when host bounds are zero or sublayer verification fails.
pub fn attach_layer_on_main_thread_sync(host_view: usize, layer: usize) -> bool {
    if host_view == 0 || layer == 0 {
        return false;
    }
    unsafe { xhvp_ios_attach_layer_to_host_sync(host_view, layer) }
}

/// Adds the sink's `CALayer` under the Flutter host view on the main thread, then runs
/// `on_complete(attach_ok)` back on the Gst thread. Never blocks xhvp-gst on the UI thread.
pub fn attach_layer_on_main_thread_async<F>(
    host_view: usize,
    layer: usize,
    lifecycle: IosAttachLifecycle,
    on_complete: F,
) where
    F: FnOnce(bool) + Send + 'static,
{
    if host_view == 0 || layer == 0 || lifecycle.is_stale() {
        spawn_on_gst_thread(move || {
            if !lifecycle.is_stale() {
                on_complete(false);
            }
        });
        return;
    }
    let ctx = Box::into_raw(Box::new(LayerAttachComplete {
        lifecycle,
        callback: Box::new(on_complete),
    }));
    unsafe {
        xhvp_ios_attach_layer_to_host_async(
            host_view,
            layer,
            Some(trampoline_layer_attach_complete),
            ctx as *mut std::ffi::c_void,
        );
    }
}
