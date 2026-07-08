//! C-ABI bridge between the native Flutter texture layer and the Rust frame
//! source ([`crate::playback::frame`]).
//!
//! The native plugin (iOS/macOS/Windows/Linux) registers a frame-ready callback
//! for a `player_id`, then on each notification pulls the latest BGRA frame into
//! its own buffer and updates the Flutter external texture. These are plain
//! `extern "C"` symbols (like the existing overlay entry points), so no
//! flutter_rust_bridge regeneration is required.

use std::ffi::c_void;

use crate::playback::frame::{frame_sink_for, FrameReadyFn};

/// Registers a frame-ready callback for `player_id`. `ctx` is opaque native
/// state passed back to `on_frame`. Safe to call before or after `load`.
///
/// # Safety
/// `ctx` must remain valid until [`xhvp_texture_unregister`] is called for the
/// same `player_id`.
#[no_mangle]
pub extern "C" fn xhvp_texture_register(player_id: i64, ctx: *mut c_void, on_frame: FrameReadyFn) {
    if let Some(sink) = frame_sink_for(player_id) {
        sink.set_callback(ctx, on_frame);
    }
}

/// Removes the frame-ready callback for `player_id`.
#[no_mangle]
pub extern "C" fn xhvp_texture_unregister(player_id: i64) {
    if let Some(sink) = frame_sink_for(player_id) {
        sink.clear_callback();
    }
}

/// Returns the latest frame geometry without copying pixels, so the native side
/// can (re)allocate its destination buffer / pixel buffer. Returns `false` when
/// no frame is available yet.
///
/// # Safety
/// `out_width`, `out_height`, `out_stride`, `out_bytes` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn xhvp_texture_frame_info(
    player_id: i64,
    out_width: *mut i32,
    out_height: *mut i32,
    out_stride: *mut i32,
    out_bytes: *mut u32,
) -> bool {
    let Some(sink) = frame_sink_for(player_id) else {
        return false;
    };
    let Some((w, h, stride, bytes)) = sink.latest_geometry() else {
        return false;
    };
    if !out_width.is_null() {
        *out_width = w;
    }
    if !out_height.is_null() {
        *out_height = h;
    }
    if !out_stride.is_null() {
        *out_stride = stride;
    }
    if !out_bytes.is_null() {
        *out_bytes = bytes as u32;
    }
    true
}

/// Copies the latest BGRA frame into `dst` (capacity `dst_len` bytes). Returns
/// `false` if there is no frame or `dst` is too small. On success the geometry
/// out-params describe the copied frame.
///
/// # Safety
/// `dst` must point to at least `dst_len` writable bytes; the out-params must be
/// valid pointers.
#[no_mangle]
pub unsafe extern "C" fn xhvp_texture_copy_latest(
    player_id: i64,
    dst: *mut u8,
    dst_len: u32,
    out_width: *mut i32,
    out_height: *mut i32,
    out_stride: *mut i32,
) -> bool {
    if dst.is_null() {
        return false;
    }
    let Some(sink) = frame_sink_for(player_id) else {
        return false;
    };
    let buffer = std::slice::from_raw_parts_mut(dst, dst_len as usize);
    match sink.copy_latest(buffer) {
        Some((w, h, stride)) => {
            if !out_width.is_null() {
                *out_width = w;
            }
            if !out_height.is_null() {
                *out_height = h;
            }
            if !out_stride.is_null() {
                *out_stride = stride;
            }
            true
        }
        None => false,
    }
}
