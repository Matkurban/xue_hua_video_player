//! iOS 主线程 UIKit / Core Animation 辅助函数（CALayer 附着、VideoOverlay 绑定）/
//! iOS main-thread UIKit / Core Animation helpers (CALayer attach, VideoOverlay bind).
//!
//! 通过 Swift 提供的 C 符号在主线程附着 sink CALayer、绑定 overlay 句柄，
//! 并在 Gst 线程上回调完成状态。
//!
//! Attaches sink CALayer and binds overlay handles on the main thread via Swift C symbols,
//! with completion callbacks marshaled back to the Gst thread.

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

/// 在主线程释放 +1 retain 的 sink layer（dealloc 必须在主线程）/
/// Releases a +1 retained sink layer on the main thread (dealloc must be main).
///
/// # 参数 / Parameters
/// - `layer` — CALayer 指针；`0` 为 no-op / CALayer pointer; `0` is no-op
pub fn release_layer_on_main_thread(layer: usize) {
    if layer != 0 {
        unsafe { xhvp_ios_release_layer_main(layer) }
    }
}

/// 在主线程从宿主视图移除 sink CALayer / Removes the sink CALayer(s) from the host view on the main thread.
///
/// # 参数 / Parameters
/// - `host_view` — Flutter 宿主 `UIView` 指针 / Flutter host `UIView` pointer
pub fn detach_sink_layers_on_main_thread(host_view: usize) {
    if host_view != 0 {
        unsafe { xhvp_ios_detach_sink_layers(host_view) }
    }
}

/// 读取 `AVSampleBufferDisplayLayer` 状态（0 未知，1 渲染中，2 失败，-1 非 display layer）与错误码 /
/// Reads the `AVSampleBufferDisplayLayer` status (0 unknown, 1 rendering, 2
/// failed, -1 not a display layer) and error code for on-device diagnostics.
///
/// # 参数 / Parameters
/// - `layer` — display layer 指针 / display layer pointer
///
/// # 返回值 / Returns
/// - `(status, error_code)` 元组 / `(status, error_code)` tuple
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

/// 在 UIKit 主线程执行 `set_window_handle`（总线 sync / glimagesink 回退）/
/// Runs `set_window_handle` on the UIKit main thread (bus sync / glimagesink fallback).
///
/// # 参数 / Parameters
/// - `sink` — video sink 元素 / video sink element
/// - `handle` — 窗口/宿主句柄 / window/host handle
///
/// # 返回值 / Returns
/// - `Ok(())` 主线程绑定已调度并完成 / `Ok(())` after main-thread bind completes
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

/// Flutter 宿主 `UIView` 是否具有非零布局 bounds / True when the Flutter host `UIView` has non-zero layout bounds.
///
/// # 参数 / Parameters
/// - `host_view` — 宿主视图指针 / host view pointer
///
/// # 返回值 / Returns
/// - `false` 当 `host_view == 0` 或 bounds 为零 / `false` when `host_view == 0` or bounds are zero
pub fn host_view_ready_for_attach(host_view: usize) -> bool {
    if host_view == 0 {
        return false;
    }
    unsafe { xhvp_ios_host_view_has_bounds(host_view) }
}

/// 在主线程同步将 sink `CALayer` 添加为宿主子层（仅 resize；首次附着用异步路径）/
/// Adds the sink's `CALayer` under the Flutter host view on the main thread synchronously
/// (resize-only; first attach uses [`attach_layer_on_main_thread_async`]).
///
/// # 参数 / Parameters
/// - `host_view` — 宿主视图指针 / host view pointer
/// - `layer` — sink CALayer 指针 / sink CALayer pointer
///
/// # 返回值 / Returns
/// - `false` 当 bounds 为零或子层校验失败 / `false` when host bounds are zero or sublayer verification fails
pub fn attach_layer_on_main_thread_sync(host_view: usize, layer: usize) -> bool {
    if host_view == 0 || layer == 0 {
        return false;
    }
    unsafe { xhvp_ios_attach_layer_to_host_sync(host_view, layer) }
}

/// 在主线程异步添加 sink `CALayer`，完成后在 Gst 线程调用 `on_complete(attach_ok)` /
/// Adds the sink's `CALayer` under the Flutter host view on the main thread, then runs
/// `on_complete(attach_ok)` back on the Gst thread. Never blocks xhvp-gst on the UI thread.
///
/// # 参数 / Parameters
/// - `host_view` — 宿主视图指针 / host view pointer
/// - `layer` — sink CALayer 指针 / sink CALayer pointer
/// - `on_complete` — Gst 线程上的完成回调 / completion callback on Gst thread
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
