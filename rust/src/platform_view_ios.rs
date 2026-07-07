//! iOS main-thread UIKit / Core Animation helpers (CALayer attach, VideoOverlay bind).

use std::cell::RefCell;

use anyhow::Result;
use gstreamer as gst;

use crate::video::{clear_overlay_window_handle, set_overlay_window_handle};

extern "C" {
    fn xhvp_dispatch_sync_main_fn(fun: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>, ctx: *mut std::ffi::c_void);
    fn xhvp_ios_attach_layer_to_host(host_view: usize, layer: usize);
}

thread_local! {
    static MAIN_THREAD_BIND: RefCell<Option<(gst::Element, usize)>> = const { RefCell::new(None) };
    static MAIN_THREAD_ATTACH: RefCell<Option<(usize, usize)>> = const { RefCell::new(None) };
}

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

/// Runs `set_window_handle` on the UIKit main thread (bus sync / Gst-thread rebind).
pub fn bind_overlay_on_main_thread(sink: &gst::Element, handle: usize) -> Result<()> {
    MAIN_THREAD_BIND.with(|cell| {
        *cell.borrow_mut() = Some((sink.clone(), handle));
    });
    unsafe {
        xhvp_dispatch_sync_main_fn(Some(trampoline_bind_overlay), std::ptr::null_mut());
    }
    Ok(())
}

unsafe extern "C" fn trampoline_attach_layer(_ctx: *mut std::ffi::c_void) {
    let _ = _ctx;
    MAIN_THREAD_ATTACH.with(|cell| {
        if let Some((host, layer)) = cell.borrow_mut().take() {
            xhvp_ios_attach_layer_to_host(host, layer);
        }
    });
}

/// Adds the sink's `CALayer` under the Flutter host view on the main thread.
pub fn attach_layer_on_main_thread(host_view: usize, layer: usize) {
    if host_view == 0 || layer == 0 {
        return;
    }
    MAIN_THREAD_ATTACH.with(|cell| {
        *cell.borrow_mut() = Some((host_view, layer));
    });
    unsafe {
        xhvp_dispatch_sync_main_fn(Some(trampoline_attach_layer), std::ptr::null_mut());
    }
}
