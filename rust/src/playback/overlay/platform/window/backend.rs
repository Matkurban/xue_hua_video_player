//! 桌面（Win/Linux/macOS）VideoOverlay 后端 / Desktop (Win/Linux/macOS) VideoOverlay backends.
//!
//! 无状态辅助函数：应用窗口句柄、同步渲染矩形。macOS 播放经 appsink 纹理路径，
//! 这些 API 主要服务 Win/Linux 桌面 overlay；macOS 保留 session 占位以统一 `VideoSurface`。
//!
//! Stateless helpers for window-handle overlay on Win/Linux. macOS playback uses the
//! appsink texture path; these APIs mainly serve Win/Linux while macOS shares the session type.

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::gst::spawn_on_gst_thread;
use crate::playback::gst::{
    clear_overlay_window_handle, set_overlay_render_rectangle, set_overlay_window_handle,
};
use crate::playback::shell::PipelineShell;

use crate::playback::overlay::video_overlay::VideoOverlayBackend;

/// 桌面 overlay 操作 — 由 [`crate::playback::surface::VideoSurface`] 委托 / Desktop overlay operations delegated from [`crate::playback::surface::VideoSurface`].
#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
pub struct DesktopOverlayBackend;

#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
impl VideoOverlayBackend for DesktopOverlayBackend {
    fn stored_handle(&self) -> &Mutex<Option<usize>> {
        unreachable!(
            "DesktopOverlayBackend is a stateless delegate; use VideoSurface stored handle"
        )
    }
}

/// 将原生窗口句柄应用到 video sink 并更新缓存 / Applies native window handle to video sink and updates cache.
///
/// # 参数 / Parameters
/// - `video_sink` — GStreamer video sink 元素 / GStreamer video sink element
/// - `handle` — 窗口句柄；`0` 清除 / window handle; `0` clears
/// - `stored` — 句柄缓存槽 / handle cache slot
///
/// # 返回值 / Returns
/// - `Ok(())` 应用或清除成功 / `Ok(())` on successful apply or clear
#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
pub fn apply_overlay_handle(
    video_sink: &gst::Element,
    handle: usize,
    stored: &Mutex<Option<usize>>,
) -> Result<()> {
    if handle == 0 {
        stored.lock().take();
    } else {
        *stored.lock() = Some(handle);
    }

    if handle == 0 {
        clear_overlay_window_handle(video_sink)?;
    } else {
        set_overlay_window_handle(video_sink, handle)?;
    }
    Ok(())
}

#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
impl DesktopOverlayBackend {
    pub fn rebind_cached_overlay(
        stored: &Mutex<Option<usize>>,
        shell: &PipelineShell,
    ) -> Result<()> {
        if let Some(handle) = *stored.lock() {
            apply_overlay_handle(shell.video_sink(), handle, stored)?;
        }
        Ok(())
    }

    pub fn schedule_rectangle_sync(
        stored: Arc<Mutex<Option<usize>>>,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        spawn_on_gst_thread(move || {
            let guard = shell.lock();
            if width > 0 && height > 0 {
                guard.apply_overlay_render_rectangle(width, height);
            } else if stored.lock().is_some() {
                guard.expose_video_overlay();
            }
        });
    }
}
