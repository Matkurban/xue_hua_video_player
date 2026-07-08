//! C-ABI 桥接：原生 Flutter 纹理层 ↔ Rust 帧源（[`crate::playback::frame`]）/
//! C-ABI bridge between the native Flutter texture layer and the Rust frame
//! source ([`crate::playback::frame`]).
//!
//! 原生插件（iOS/macOS/Windows/Linux）为 `player_id` 注册帧就绪回调，
//! 每次通知时拉取最新 BGRA 帧并更新 Flutter 外部纹理。
//! 均为 plain `extern "C"` 符号（与现有 overlay 入口一致），无需 flutter_rust_bridge 重新生成。
//!
//! The native plugin (iOS/macOS/Windows/Linux) registers a frame-ready callback
//! for a `player_id`, then on each notification pulls the latest BGRA frame into
//! its own buffer and updates the Flutter external texture. These are plain
//! `extern "C"` symbols (like the existing overlay entry points), so no
//! flutter_rust_bridge regeneration is required.

use std::ffi::c_void;

use crate::playback::frame::{frame_sink_for, FrameReadyFn};

/// 为 `player_id` 注册帧就绪回调；`ctx` 为不透明原生状态，回传给 `on_frame` /
/// Registers a frame-ready callback for `player_id`. `ctx` is opaque native
/// state passed back to `on_frame`. Safe to call before or after `load`.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器实例 ID / player instance ID
/// - `ctx` — 不透明原生上下文指针 / opaque native context pointer
/// - `on_frame` — 帧就绪时调用的 C 回调 / C callback invoked when a frame is ready
///
/// # Safety
/// `ctx` 必须保持有效直至对同一 `player_id` 调用 [`xhvp_texture_unregister`] /
/// `ctx` must remain valid until [`xhvp_texture_unregister`] is called for the
/// same `player_id`.
#[no_mangle]
pub extern "C" fn xhvp_texture_register(player_id: i64, ctx: *mut c_void, on_frame: FrameReadyFn) {
    if let Some(sink) = frame_sink_for(player_id) {
        sink.set_callback(ctx, on_frame);
    }
}

/// 移除 `player_id` 的帧就绪回调 / Removes the frame-ready callback for `player_id`.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器实例 ID / player instance ID
#[no_mangle]
pub extern "C" fn xhvp_texture_unregister(player_id: i64) {
    if let Some(sink) = frame_sink_for(player_id) {
        sink.clear_callback();
    }
}

/// 返回最新帧几何信息（不拷贝像素），供原生侧重分配缓冲区 /
/// Returns the latest frame geometry without copying pixels, so the native side
/// can (re)allocate its destination buffer / pixel buffer.
///
/// # 参数 / Parameters
/// - `player_id` — 播放器实例 ID / player instance ID
/// - `out_width` — 输出宽度指针 / output width pointer
/// - `out_height` — 输出高度指针 / output height pointer
/// - `out_stride` — 输出行跨度指针 / output stride pointer
/// - `out_bytes` — 输出字节数指针 / output byte count pointer
///
/// # 返回值 / Returns
/// - `true` 当帧可用且几何已写入；尚无帧时 `false` / `true` when frame available and geometry written; `false` when no frame yet
///
/// # Safety
/// `out_width`、`out_height`、`out_stride`、`out_bytes` 必须为有效指针 /
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

/// 将最新 BGRA 帧拷贝到 `dst`（容量 `dst_len` 字节）/
/// Copies the latest BGRA frame into `dst` (capacity `dst_len` bytes).
///
/// # 参数 / Parameters
/// - `player_id` — 播放器实例 ID / player instance ID
/// - `dst` — 目标缓冲区指针 / destination buffer pointer
/// - `dst_len` — 目标缓冲区容量（字节）/ destination buffer capacity in bytes
/// - `out_width` — 成功时输出宽度 / output width on success
/// - `out_height` — 成功时输出高度 / output height on success
/// - `out_stride` — 成功时输出行跨度 / output stride on success
///
/// # 返回值 / Returns
/// - `false` 无帧或 `dst` 过小；成功时几何写入 out 参数并返回 `true` /
///   `false` if no frame or `dst` too small; on success writes geometry to out-params and returns `true`
///
/// # Safety
/// `dst` 必须指向至少 `dst_len` 字节可写内存；out 参数必须为有效指针 /
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
