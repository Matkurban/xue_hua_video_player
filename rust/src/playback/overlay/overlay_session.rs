//! 统一 overlay 会话接口 — 加载预卷 + 表面通知/应用。
//!
//! [`OverlaySession`] 抽象各平台在 URI 加载、原生视图绑定、GStreamer VideoOverlay
//! 应用阶段的策略差异；[`load_preroll`] 提供共享的加载路径辅助函数。
//!
//! Unified overlay session interface — load preroll + surface notify/apply.
//!
//! [`OverlaySession`] abstracts per-platform policy for URI load, native view bind,
//! and GStreamer VideoOverlay apply; [`load_preroll`] holds shared load-path helpers.

use std::sync::Arc;

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

use crate::playback::overlay::preroll::{decide_preroll_action, PrerollAction};
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
use crate::playback::surface::VideoSurface;

/// 平台 overlay 会话 — 加载预卷、通知缓存与 Gst 应用/附着 / Platform overlay session — load preroll, notify cache, and Gst apply/attach.
///
/// 每个平台实现（Android、iOS、macOS、桌面）通过此 trait 向 [`VideoSurface`] 暴露
/// 一致的绑定生命周期与预卷门控语义。
///
/// Each platform implementation (Android, iOS, macOS, desktop) exposes a consistent
/// bind lifecycle and preroll-gating semantics to [`VideoSurface`] through this trait.
pub trait OverlaySession: Send + Sync {
    /// 加载路径是否允许进入预卷门控 / Whether the load path may enter preroll gating.
    ///
    /// # 参数 / Parameters
    /// - `surface_overlay_ready` — [`VideoSurface`] 报告的 overlay 就绪状态（通常含句柄缓存与绑定）/
    ///   overlay readiness reported by [`VideoSurface`] (typically handle cache + bind)
    ///
    /// # 返回值 / Returns
    /// - `true` 当该平台认为可继续加载预卷；iOS 恒为 `false`（附着延后到 session）/
    ///   `true` when this platform may proceed with load preroll; iOS always `false` (attach deferred to session)
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool;

    /// 在 URI/资源加载后应用加载路径预卷策略 / Applies load-path preroll policy after URI/asset load.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 当前管线壳层（只读快照/状态变更）/ current pipeline shell (read snapshots / change state)
    /// - `surface` — 关联的 [`VideoSurface`]，用于尺寸与句柄 / associated [`VideoSurface`] for dimensions and handle
    /// - `defer_log` — 门控未通过时写入的诊断日志消息 / diagnostic log message when gate defers preroll
    ///
    /// # 返回值 / Returns
    /// - `Ok(())` 成功或已记录 defer；平台可暂停管线或仅打日志 / `Ok(())` on success or logged defer; platform may pause pipeline or log only
    ///
    /// # 错误 / Errors
    /// - GStreamer 状态同步或 overlay 刷新失败 / GStreamer state sync or overlay refresh failure
    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface: &VideoSurface,
        defer_log: &str,
    ) -> Result<()>;

    /// overlay 是否已在 GStreamer 侧完成绑定 / Whether the overlay is bound on the GStreamer side.
    ///
    /// # 返回值 / Returns
    /// - `true` 当 `set_window_handle` / CALayer 附着 / native window 已生效 /
    ///   `true` when `set_window_handle` / CALayer attach / native window is active
    fn is_bound(&self) -> bool;

    /// 绑定路径预卷是否就绪 / Whether bind-path preroll may proceed.
    ///
    /// # 参数 / Parameters
    /// - `has_cached_handle` — 原生宿主视图/窗口句柄是否已缓存 / whether the native host view/window handle is cached
    ///
    /// # 返回值 / Returns
    /// - `true` 当句柄与平台绑定状态均满足预卷条件 / `true` when handle and platform bind state satisfy preroll
    fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool;

    /// 管线壳层重建后使 overlay 状态失效 / Invalidates overlay state after pipeline shell rebuild.
    ///
    /// 典型行为：清除 `overlay_bound`、递增 generation、重置 iOS 附着标志等。
    ///
    /// Typical behavior: clear `overlay_bound`, bump generation, reset iOS attach flags, etc.
    fn mark_shell_rebuilt(&self);

    /// 缓存最近一次布局/report 的渲染矩形尺寸 / Caches the most recent layout/reported render rectangle size.
    ///
    /// # 参数 / Parameters
    /// - `width` — 像素宽度；`<= 0` 时多数平台忽略 / pixel width; most platforms ignore `<= 0`
    /// - `height` — 像素高度；`<= 0` 时多数平台忽略 / pixel height; most platforms ignore `<= 0`
    fn set_cached_dimensions(&self, width: i32, height: i32);

    /// 返回缓存的渲染矩形尺寸 / Returns the cached render rectangle dimensions.
    ///
    /// # 返回值 / Returns
    /// - `(width, height)` 像素对；未设置时为 `(0, 0)` / `(width, height)` in pixels; `(0, 0)` when unset
    fn cached_dimensions(&self) -> (i32, i32);

    /// 在新 `video_sink` 上重新绑定已缓存的 overlay / Rebinds the cached overlay onto a new `video_sink`.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 已安装新 sink 的管线壳层 / pipeline shell with the new sink installed
    /// - `stored` — 共享的原生句柄缓存槽 / shared native handle cache slot
    ///
    /// # 返回值 / Returns
    /// - `Ok(())` 无句柄时通常为 no-op / `Ok(())`; usually no-op when no handle cached
    ///
    /// # 错误 / Errors
    /// - GStreamer overlay 句柄/矩形应用失败 / GStreamer overlay handle/rectangle apply failure
    fn rebind_cached_overlay(
        &self,
        shell: &PipelineShell,
        stored: Arc<Mutex<Option<usize>>>,
    ) -> Result<()>;

    /// 缓存原生句柄与尺寸（iOS/macOS 上不在此步做 Gst 附着）/ Cache native handle and dimensions (no Gst attach on iOS/macOS).
    ///
    /// # 参数 / Parameters
    /// - `stored` — 共享句柄缓存互斥槽 / shared handle cache mutex slot
    /// - `handle` — 原生宿主指针/句柄；`0` 表示销毁 / native host pointer/handle; `0` means destroyed
    /// - `width` — 宿主视图宽度（像素）/ host view width in pixels
    /// - `height` — 宿主视图高度（像素）/ host view height in pixels
    ///
    /// # 返回值 / Returns
    /// - `Ok(())` 缓存更新成功 / `Ok(())` on successful cache update
    ///
    /// # 错误 / Errors
    /// - 原生窗口释放或缓存失败（Android）/ native window release or cache failure (Android)
    fn cache_notify(
        &self,
        stored: &Arc<Mutex<Option<usize>>>,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()>;

    /// 布局时执行 GStreamer overlay 应用/附着 / Layout-time Gst overlay apply/attach.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 管线壳层（可在 Gst 线程加锁）/ pipeline shell (lockable on Gst thread)
    /// - `stored` — 原生句柄缓存 / native handle cache
    /// - `surface` — 用于预卷恢复与切换的 [`VideoSurface`] 克隆 / [`VideoSurface`] clone for preroll resume and switch
    /// - `width` — 目标渲染宽度 / target render width
    /// - `height` — 目标渲染高度 / target render height
    /// - `play_intent` — 播放意图（`desired_playing`、replay/swap 状态）/ play intent (`desired_playing`, replay/swap state)
    ///
    /// # 返回值 / Returns
    /// - `Ok(())` 已调度或完成应用 / `Ok(())` when apply is scheduled or completed
    ///
    /// # 错误 / Errors
    /// - 主线程派发、CALayer 附着或 GStreamer 绑定失败 / main-thread dispatch, CALayer attach, or GStreamer bind failure
    fn apply_gstreamer(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        stored: Arc<Mutex<Option<usize>>>,
        surface: VideoSurface,
        width: i32,
        height: i32,
        play_intent: OverlayPlayIntent,
    ) -> Result<()>;

    /// Android JNI 入口：在一次调用中完成缓存并调度绑定 / Android JNI entry: cache + schedule bind in one call.
    ///
    /// 默认实现为 no-op；[`AndroidOverlaySession`] 覆盖以在 Gst 线程异步应用 overlay。
    ///
    /// Default is no-op; [`AndroidOverlaySession`] overrides to apply overlay asynchronously on Gst thread.
    ///
    /// # 参数 / Parameters
    /// - `stored` — 句柄缓存槽 / handle cache slot
    /// - `handle` — `ANativeWindow` 句柄；`0` 触发清除 / `ANativeWindow` handle; `0` triggers clear
    /// - `width` — Surface 宽度 / surface width
    /// - `height` — Surface 高度 / surface height
    /// - `shell` — 管线壳层 / pipeline shell
    /// - `surface` — 关联 [`VideoSurface`] / associated [`VideoSurface`]
    /// - `play_intent` — 绑定后预卷恢复意图 / preroll-resume intent after bind
    ///
    /// # 返回值 / Returns
    /// - `Ok(())` 已缓存并调度（或已清除）/ `Ok(())` after cache+schedule (or clear)
    fn notify_surface_with_shell(
        &self,
        stored: Arc<Mutex<Option<usize>>>,
        handle: i64,
        width: i32,
        height: i32,
        shell: Arc<Mutex<PipelineShell>>,
        surface: VideoSurface,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        let _ = (stored, handle, width, height, shell, surface, play_intent);
        Ok(())
    }
}

/// 平台 session 适配器共用的加载预卷辅助函数 / Shared load-preroll helpers used by platform session adapters.
pub(crate) mod load_preroll {
    use super::*;

    /// Android 加载路径预卷：可能暂停并刷新 overlay / Android load-path preroll: may pause and refresh overlay.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 管线壳层 / pipeline shell
    /// - `gate_ready` — 门控是否通过 / whether the gate passed
    /// - `surface` — 用于 overlay 刷新的 [`VideoSurface`] / [`VideoSurface`] for overlay refresh
    /// - `defer_log` — defer 时记录的日志 / log message on defer
    ///
    /// # 返回值 / Returns
    /// - `Ok(())` 完成、defer 或 noop / `Ok(())` on complete, defer, or noop
    #[cfg(target_os = "android")]
    pub fn android_apply_load_preroll(
        shell: &PipelineShell,
        gate_ready: bool,
        surface: &VideoSurface,
        defer_log: &str,
    ) -> Result<()> {
        use crate::playback::overlay::platform::android::android_pause_preroll_with_refresh;

        let snapshot = shell.snapshot();
        match decide_preroll_action(snapshot, false, gate_ready) {
            PrerollAction::PausePreroll => {
                android_pause_preroll_with_refresh(shell, surface, None)?;
            }
            PrerollAction::Defer => {
                crate::diag::logcat_info(defer_log);
            }
            PrerollAction::Noop | PrerollAction::ResumePlaying => {}
        }
        Ok(())
    }

    /// iOS 加载路径预滚：仅记录日志，CALayer 附着由 [`IosOverlaySession`] 负责 / iOS load-path preroll: logs only; CALayer attach is owned by [`IosOverlaySession`].
    ///
    /// # 参数 / Parameters
    /// - `gate_ready` — 门控状态（iOS 恒为 false）/ gate state (always false on iOS)
    /// - `defer_log` — defer 日志消息 / defer log message
    ///
    /// # 返回值 / Returns
    /// - 恒为 `Ok(())` / always `Ok(())`
    pub fn ios_apply_load_preroll(gate_ready: bool, defer_log: &str) -> Result<()> {
        if gate_ready {
            log::debug!("gst: ios layer attach deferred to IosOverlaySession after load");
        } else {
            log::info!("{defer_log}");
        }
        Ok(())
    }

    /// 桌面/macOS 加载路径预卷：门控通过时同步暂停到 Paused / Desktop/macOS load-path preroll: sync-pause to Paused when gate passes.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 管线壳层 / pipeline shell
    /// - `gate_ready` — 门控是否通过 / whether the gate passed
    ///
    /// # 返回值 / Returns
    /// - `Ok(())` 完成或 noop / `Ok(())` on complete or noop
    pub fn desktop_apply_load_preroll(shell: &PipelineShell, gate_ready: bool) -> Result<()> {
        let snapshot = shell.snapshot();
        if decide_preroll_action(snapshot, false, gate_ready) == PrerollAction::PausePreroll {
            shell.set_state_sync(gst::State::Paused)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod fake {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    /// 用于测试 overlay 会话策略方法的替身实现 / Test double for overlay session policy methods.
    pub struct FakeOverlaySession {
        /// 模拟 GStreamer 绑定状态 / simulated GStreamer bind state
        pub bound: AtomicBool,
        /// 门控是否将 `surface_overlay_ready` 透传 / whether gate passes through `surface_overlay_ready`
        pub gate_ready: bool,
        /// 预卷是否要求 `is_bound()` / whether preroll requires `is_bound()`
        pub preroll_ready: bool,
    }

    impl FakeOverlaySession {
        /// 创建可配置门控与预卷行为的假 session / Creates a fake session with configurable gate and preroll behavior.
        ///
        /// # 参数 / Parameters
        /// - `gate_ready` — 门控配置 / gate configuration
        /// - `preroll_ready` — 预卷是否要求绑定 / whether preroll requires bind
        pub fn new(gate_ready: bool, preroll_ready: bool) -> Self {
            Self {
                bound: AtomicBool::new(false),
                gate_ready,
                preroll_ready,
            }
        }
    }

    impl OverlaySession for FakeOverlaySession {
        fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
            if self.gate_ready {
                surface_overlay_ready
            } else {
                false
            }
        }

        fn apply_load_preroll(
            &self,
            _shell: &PipelineShell,
            _surface: &VideoSurface,
            _defer_log: &str,
        ) -> Result<()> {
            Ok(())
        }

        fn is_bound(&self) -> bool {
            self.bound.load(Ordering::SeqCst)
        }

        fn overlay_ready_for_preroll(&self, has_cached_handle: bool) -> bool {
            has_cached_handle && self.preroll_ready && self.is_bound()
        }

        fn mark_shell_rebuilt(&self) {
            self.bound.store(false, Ordering::SeqCst);
        }

        fn set_cached_dimensions(&self, _width: i32, _height: i32) {}

        fn cached_dimensions(&self) -> (i32, i32) {
            (0, 0)
        }

        fn rebind_cached_overlay(
            &self,
            _shell: &PipelineShell,
            _stored: Arc<Mutex<Option<usize>>>,
        ) -> Result<()> {
            Ok(())
        }

        fn cache_notify(
            &self,
            _stored: &Arc<Mutex<Option<usize>>>,
            _handle: i64,
            _width: i32,
            _height: i32,
        ) -> Result<()> {
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
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::fake::FakeOverlaySession;
    use super::*;

    #[test]
    fn fake_gate_passes_surface_ready_when_configured() {
        let session = FakeOverlaySession::new(true, true);
        assert!(session.gate_ready_for_load(true));
        assert!(!session.gate_ready_for_load(false));
    }

    #[test]
    fn fake_gate_always_false_when_configured() {
        let session = FakeOverlaySession::new(false, true);
        assert!(!session.gate_ready_for_load(true));
        assert!(!session.gate_ready_for_load(false));
    }

    #[test]
    fn fake_preroll_requires_bind_when_configured() {
        let session = FakeOverlaySession::new(true, true);
        assert!(!session.overlay_ready_for_preroll(true));
        session.bound.store(true, Ordering::SeqCst);
        assert!(session.overlay_ready_for_preroll(true));
    }

    #[cfg(all(
        not(target_os = "android"),
        not(target_os = "ios"),
        not(target_os = "macos")
    ))]
    #[test]
    fn desktop_gate_passes_surface_ready() {
        use crate::playback::overlay::DesktopOverlaySession;
        let session = DesktopOverlaySession::new();
        assert!(session.gate_ready_for_load(true));
        assert!(!session.gate_ready_for_load(false));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_texture_gate_always_ready() {
        use crate::playback::overlay::DesktopOverlaySession;
        let session = DesktopOverlaySession::new();
        assert!(session.gate_ready_for_load(false));
        assert!(session.overlay_ready_for_preroll(false));
    }

    #[cfg(target_os = "android")]
    #[test]
    fn android_gate_passes_surface_ready() {
        use crate::playback::overlay::AndroidOverlaySession;
        let session = AndroidOverlaySession::new();
        assert!(session.gate_ready_for_load(true));
        assert!(!session.gate_ready_for_load(false));
    }

    #[cfg(target_os = "ios")]
    #[test]
    fn ios_gate_always_false() {
        use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};

        use crate::playback::overlay::IosOverlaySession;
        let session = IosOverlaySession::new(
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicBool::new(true)),
            Arc::new(AtomicU64::new(0)),
        );
        assert!(!session.gate_ready_for_load(true));
        assert!(!session.gate_ready_for_load(false));
    }
}
