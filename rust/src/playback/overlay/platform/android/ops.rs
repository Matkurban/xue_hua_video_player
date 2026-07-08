//! Android VideoOverlay 辅助函数（`glimagesink` / native window）/ Android VideoOverlay helpers (`glimagesink` / native window).
//!
//! 在 Gst 线程上缓存 `ANativeWindow`、刷新 overlay 矩形，并在加载/绑定路径执行 Paused 预卷。
//!
//! Caches `ANativeWindow` on the Gst thread, refreshes overlay rectangles, and runs Paused preroll on load/bind paths.

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::gst::expose_overlay;
use crate::playback::shell::PipelineShell;

/// 缓存或释放 Android 原生窗口句柄 / Caches or releases an Android native window handle.
///
/// # 参数 / Parameters
/// - `stored` — 句柄缓存互斥槽 / handle cache mutex slot
/// - `handle` — `ANativeWindow` 指针；`0` 释放旧窗口 / `ANativeWindow` pointer; `0` releases the old window
///
/// # 返回值 / Returns
/// - `Ok(())` 缓存或释放成功 / `Ok(())` on successful cache or release
pub fn cache_android_native_window(stored: &Mutex<Option<usize>>, handle: usize) -> Result<()> {
    if handle == 0 {
        if let Some(old) = stored.lock().take() {
            crate::platform::android::release_native_window(old);
        }
        return Ok(());
    }
    let mut guard = stored.lock();
    if let Some(old) = *guard {
        if old != handle {
            crate::platform::android::release_native_window(old);
        }
    }
    *guard = Some(handle);
    Ok(())
}

/// 在 Gst 线程上重新绑定已缓存原生窗口（Android）/ Rebinds the cached native window on the Gst thread (Android).
///
/// # 参数 / Parameters
/// - `shell` — 管线壳层 / pipeline shell
/// - `handle` — 原生窗口句柄 / native window handle
/// - `width` — 渲染宽度 / render width
/// - `height` — 渲染高度 / render height
/// - `reason` — 诊断日志原因标签 / diagnostic log reason tag
///
/// # 返回值 / Returns
/// - `Ok(())` 句柄与矩形已应用并已 expose / `Ok(())` after handle and rectangle applied and exposed
pub fn refresh_mobile_overlay_on_gst(
    shell: &PipelineShell,
    handle: usize,
    width: i32,
    height: i32,
    reason: &str,
) -> Result<()> {
    shell.apply_overlay_window_handle(handle)?;
    shell.apply_overlay_render_rectangle(width, height);
    shell.expose_video_overlay();
    crate::diag::logcat_info(&format!("gst: overlay refresh {reason} ({width}x{height})"));
    Ok(())
}

/// Paused 预卷 + overlay 刷新 — 加载与绑定路径共用 / Paused preroll + overlay refresh — shared by load and bind paths.
///
/// # 参数 / Parameters
/// - `shell` — 管线壳层 / pipeline shell
/// - `surface` — 提供句柄与缓存尺寸的 [`VideoSurface`] / [`VideoSurface`] for handle and cached dimensions
/// - `log_prefix` — 可选日志前缀 / optional log prefix
///
/// # 返回值 / Returns
/// - `Ok(())` 暂停并刷新成功（无句柄时仅暂停）/ `Ok(())` after pause and refresh (pause only when no handle)
pub fn android_pause_preroll_with_refresh(
    shell: &PipelineShell,
    surface: &crate::playback::surface::VideoSurface,
    log_prefix: Option<&str>,
) -> Result<()> {
    if let Some(msg) = log_prefix {
        crate::diag::logcat_info(msg);
    }
    shell.set_state_sync(gst::State::Paused)?;
    if let Some(handle) = *surface.stored_handle().lock() {
        let (width, height) = surface.cached_dimensions();
        refresh_mobile_overlay_on_gst(shell, handle, width, height, "after Paused preroll")?;
    }
    Ok(())
}
