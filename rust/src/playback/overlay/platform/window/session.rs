//! Linux / Windows / macOS overlay 会话 / Linux / Windows / macOS overlay sessions.
//!
//! [`DesktopOverlaySession`] 实现 [`OverlaySession`]，处理 Win/Linux 窗口句柄绑定。
//! macOS 经 Flutter 外部 Texture（appsink）播放，gate 恒为就绪。
//!
//! [`DesktopOverlaySession`] implements [`OverlaySession`] for Win/Linux window handles.
//! macOS uses Flutter external Texture (appsink); gates always report ready.

use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    Arc,
};

use anyhow::Result;
use parking_lot::Mutex;

use crate::playback::overlay::overlay_session::{load_preroll, OverlaySession};
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;

use super::backend::DesktopOverlayBackend;

/// 桌面 overlay 会话（含 macOS texture 占位）/ Desktop overlay session (includes macOS texture stub).
#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
#[derive(Clone)]
pub struct DesktopOverlaySession {
    overlay_bound: Arc<AtomicBool>,
    last_width: Arc<AtomicI32>,
    last_height: Arc<AtomicI32>,
}

#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
impl DesktopOverlaySession {
    /// 创建未绑定的桌面 overlay session / Creates an unbound desktop overlay session.
    pub fn new() -> Self {
        Self {
            overlay_bound: Arc::new(AtomicBool::new(false)),
            last_width: Arc::new(AtomicI32::new(0)),
            last_height: Arc::new(AtomicI32::new(0)),
        }
    }

    /// 设置 GStreamer overlay 绑定标志 / Sets the GStreamer overlay bound flag.
    pub fn set_bound(&self, bound: bool) {
        self.overlay_bound.store(bound, Ordering::SeqCst);
    }

    /// 在 Gst 线程调度渲染矩形同步 / Schedules render-rectangle sync on the Gst thread.
    pub fn schedule_rectangle_sync(
        &self,
        stored: Arc<Mutex<Option<usize>>>,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        DesktopOverlayBackend::schedule_rectangle_sync(stored, shell, width, height);
    }

    /// 应用窗口句柄；`handle != 0` 时标记 overlay 已绑定 /
    /// Applies the window handle and marks the overlay bound when `handle != 0`.
    pub fn apply_window_handle(
        &self,
        shell: &PipelineShell,
        stored: &Mutex<Option<usize>>,
        window_handle: i64,
    ) -> Result<()> {
        super::backend::apply_overlay_handle(shell.video_sink(), window_handle as usize, stored)?;
        self.set_bound(window_handle != 0);
        Ok(())
    }
}

#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
impl OverlaySession for DesktopOverlaySession {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
        #[cfg(target_os = "macos")]
        {
            let _ = surface_overlay_ready;
            return true;
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Match Android: only preroll after VideoOverlay is bound.
            surface_overlay_ready
        }
    }

    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface: &VideoSurface,
        _defer_log: &str,
    ) -> Result<()> {
        let gate_ready = self.gate_ready_for_load(surface.overlay_ready_for_preroll());
        load_preroll::desktop_apply_load_preroll(shell, gate_ready)
    }

    fn is_bound(&self) -> bool {
        self.overlay_bound.load(Ordering::SeqCst)
    }

    fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool {
        #[cfg(target_os = "macos")]
        {
            let _ = has_cached_handle;
            true
        }
        #[cfg(not(target_os = "macos"))]
        {
            has_cached_handle && self.is_bound()
        }
    }

    fn mark_shell_rebuilt(&self) {
        self.set_bound(false);
    }

    fn set_cached_dimensions(&self, width: i32, height: i32) {
        if width > 0 {
            self.last_width.store(width, Ordering::SeqCst);
        }
        if height > 0 {
            self.last_height.store(height, Ordering::SeqCst);
        }
    }

    fn cached_dimensions(&self) -> (i32, i32) {
        (
            self.last_width.load(Ordering::SeqCst),
            self.last_height.load(Ordering::SeqCst),
        )
    }

    fn rebind_cached_overlay(
        &self,
        shell: &PipelineShell,
        stored: Arc<Mutex<Option<usize>>>,
    ) -> Result<()> {
        DesktopOverlayBackend::rebind_cached_overlay(stored.as_ref(), shell)?;
        self.set_bound(stored.lock().is_some());
        Ok(())
    }

    fn cache_notify(
        &self,
        stored: &Arc<Mutex<Option<usize>>>,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()> {
        self.set_cached_dimensions(width, height);
        if handle == 0 {
            *stored.lock() = None;
            self.set_bound(false);
        } else {
            *stored.lock() = Some(handle as usize);
        }
        Ok(())
    }

    fn apply_gstreamer(
        &self,
        _shell: Arc<Mutex<PipelineShell>>,
        _stored: Arc<Mutex<Option<usize>>>,
        _surface: VideoSurface,
        _width: i32,
        _height: i32,
        _play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        Ok(())
    }
}

#[cfg(all(test, not(target_os = "android"), not(target_os = "ios")))]
mod tests {
    use super::*;

    #[test]
    fn desktop_bound_tracks_apply() {
        let session = DesktopOverlaySession::new();
        assert!(!session.is_bound());
        #[cfg(not(target_os = "macos"))]
        assert!(!session.overlay_ready_for_preroll(true));
        #[cfg(target_os = "macos")]
        assert!(session.overlay_ready_for_preroll(true));
        session.set_bound(true);
        assert!(session.overlay_ready_for_preroll(true));
        session.mark_shell_rebuilt();
        assert!(!session.is_bound());
    }
}
