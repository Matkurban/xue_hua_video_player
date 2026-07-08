//! 缓存原生 overlay 句柄的薄委托层 — 将平台 [`OverlaySession`] 接入播放管线。
//!
//! [`VideoSurface`] 持有共享句柄槽与各平台 session 实现，向上层暴露统一的
//! 绑定、预卷、尺寸同步与管线重建失效语义。
//!
//! Thin delegate for cached native overlay handles — wires platform [`OverlaySession`] into playback.
//!
//! [`VideoSurface`] holds the shared handle slot and per-platform session impls, exposing
//! unified bind, preroll, dimension sync, and shell-rebuild invalidation semantics.

use std::sync::Arc;

#[cfg(target_os = "ios")]
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

#[cfg(target_os = "android")]
use crate::gst::spawn_on_gst_thread;
#[cfg(target_os = "ios")]
use crate::platform::ios::layer::IosLayerAttachOutcome;
#[cfg(target_os = "android")]
use crate::playback::overlay::refresh_mobile_overlay_on_gst;
#[cfg(target_os = "android")]
use crate::playback::overlay::AndroidOverlaySession;
#[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
use crate::playback::overlay::DesktopOverlaySession;
#[cfg(target_os = "ios")]
use crate::playback::overlay::IosLayerBackend;
#[cfg(target_os = "ios")]
use crate::playback::overlay::IosOverlaySession;
use crate::playback::overlay::OverlaySession;
use crate::playback::overlay::VideoOverlayBackend;
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    all(not(target_os = "android"), not(target_os = "ios"))
))]
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;

/// 缓存原生 overlay 句柄 — 委托至平台 [`OverlaySession`] / Cached native overlay handle — thin delegate to platform [`OverlaySession`].
///
/// 按编译目标嵌入对应平台的 session（Android / iOS / macOS / Desktop）。
///
/// Embeds the platform session (Android / iOS / macOS / Desktop) per compile target.
pub struct VideoSurface {
    stored: Arc<Mutex<Option<usize>>>,
    #[cfg(target_os = "android")]
    session: AndroidOverlaySession,
    #[cfg(target_os = "ios")]
    session: IosOverlaySession,
    #[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
    session: DesktopOverlaySession,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl VideoOverlayBackend for VideoSurface {
    fn stored_handle(&self) -> &Mutex<Option<usize>> {
        self.stored.as_ref()
    }
}

impl VideoSurface {
    /// 创建 [`VideoSurface`]，使用外部共享的句柄缓存槽 / Creates a [`VideoSurface`] with an externally shared handle cache slot.
    ///
    /// # 参数 / Parameters
    /// - `stored` — 原生宿主视图/窗口句柄的共享互斥槽 / shared mutex slot for native host view/window handle
    ///
    /// # 返回值 / Returns
    /// - 已初始化平台 session 的 surface / surface with platform session initialized
    pub fn new(stored: Arc<Mutex<Option<usize>>>) -> Self {
        Self {
            stored,
            #[cfg(target_os = "android")]
            session: AndroidOverlaySession::new(),
            #[cfg(target_os = "ios")]
            session: IosOverlaySession::new_with_running(Arc::new(AtomicBool::new(true))),
            #[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
            session: DesktopOverlaySession::new(),
        }
    }

    /// 将 iOS replay 运行标志接入 session（管线存活期）/ Wires the iOS replay running flag into the session (pipeline lifetime).
    ///
    /// # 参数 / Parameters
    /// - `running` — 为 `false` 时使进行中的附着/应用工作失效 / when `false`, invalidates in-flight attach/apply work
    #[cfg(target_os = "ios")]
    pub fn wire_ios_replay_running(&mut self, running: Arc<AtomicBool>) {
        self.session.wire_running(running);
    }

    /// 返回底层平台 [`OverlaySession`] 的 trait 对象引用 / Returns a trait-object reference to the underlying platform [`OverlaySession`].
    ///
    /// # 返回值 / Returns
    /// - 当前编译目标对应的 session 实现 / session impl for the current compile target
    pub fn overlay_session(&self) -> &dyn OverlaySession {
        &self.session
    }

    /// 克隆 iOS overlay session（供 bus 后端注册）/ Clones the iOS overlay session (for bus backend registration).
    ///
    /// # 返回值 / Returns
    /// - 可共享的 [`IosOverlaySession`] 克隆 / shareable [`IosOverlaySession`] clone
    #[cfg(target_os = "ios")]
    pub fn ios_session(&self) -> IosOverlaySession {
        self.session.clone()
    }

    /// 返回 iOS bus 层后端注册槽 / Returns the iOS bus-layer backend registration slot.
    ///
    /// # 返回值 / Returns
    /// - 共享的 `Option<IosLayerBackend>` 互斥槽 / shared mutex slot for `Option<IosLayerBackend>`
    #[cfg(target_os = "ios")]
    pub fn ios_layer_bus_slot(&self) -> Arc<Mutex<Option<IosLayerBackend>>> {
        self.session.ios_layer_bus_slot.clone()
    }

    /// 注册 iOS Gst bus 层后端（`READY→PAUSED` 附着重试）/ Registers the iOS Gst bus-layer backend (`READY→PAUSED` attach retries).
    ///
    /// # 参数 / Parameters
    /// - `backend` — 由 [`PlaybackGstContext`] 构建的 bus 适配器 / bus adapter built from [`PlaybackGstContext`]
    #[cfg(target_os = "ios")]
    pub fn register_ios_layer_backend(&self, backend: IosLayerBackend) {
        self.session.register_ios_layer_backend(backend);
    }

    /// 创建带预装 overlay sink 槽的 surface（iOS）/ Creates a surface with a pre-installed overlay sink slot (iOS).
    #[cfg(target_os = "ios")]
    pub fn with_overlay_sink_slot(
        stored: Arc<Mutex<Option<usize>>>,
        overlay_sink: Arc<Mutex<gst::Element>>,
        running: Arc<AtomicBool>,
    ) -> Self {
        let mut session = IosOverlaySession::new_with_running(running);
        session.set_overlay_sink(overlay_sink.lock().clone());
        Self {
            stored,
            #[cfg(target_os = "android")]
            session: AndroidOverlaySession::new(),
            session,
        }
    }

    /// 更新 session 中的 overlay sink 元素（iOS）/ Updates the overlay sink element in the session (iOS).
    #[cfg(target_os = "ios")]
    pub fn set_overlay_sink_slot(&mut self, element: gst::Element) {
        self.session.set_overlay_sink(element);
    }

    /// 返回句柄缓存槽的共享克隆 / Returns a shared clone of the handle cache slot.
    ///
    /// # 返回值 / Returns
    /// - `Arc<Mutex<Option<usize>>>` 句柄缓存 / handle cache
    pub fn stored_handle(&self) -> Arc<Mutex<Option<usize>>> {
        self.stored.clone()
    }

    /// 是否已缓存非零原生句柄 / Whether a non-zero native handle is cached.
    ///
    /// # 返回值 / Returns
    /// - `true` 当互斥槽为 `Some` / `true` when the mutex slot is `Some`
    pub fn has_cached_handle(&self) -> bool {
        self.stored.lock().is_some()
    }

    /// overlay 是否已在 GStreamer 侧绑定 / Whether the overlay is bound on the GStreamer side.
    ///
    /// # 返回值 / Returns
    /// - 委托至平台 session 的 [`OverlaySession::is_bound`] / delegates to platform [`OverlaySession::is_bound`]
    pub fn is_overlay_bound_on_gst(&self) -> bool {
        self.session.is_bound()
    }

    /// 绑定路径预卷是否就绪 / Whether bind-path preroll may proceed.
    ///
    /// # 返回值 / Returns
    /// - 委托至 session，并传入 [`has_cached_handle`] / delegates to session with [`has_cached_handle`]
    pub fn overlay_ready_for_preroll(&self) -> bool {
        self.session
            .overlay_ready_for_preroll(self.has_cached_handle())
    }

    /// 调度 iOS CALayer 异步附着（load/play 路径）/ Schedules async iOS CALayer attach (load/play path).
    ///
    /// # 参数 / Parameters
    /// - `shell` — 管线壳层 / pipeline shell
    /// - `play_intent` — 附着完成后的播放意图 / play intent after attach completes
    ///
    /// # 返回值 / Returns
    /// - [`IosLayerAttachOutcome`]：已调度、层未就绪或已跳过 / scheduled, layer not ready, or skipped
    #[cfg(target_os = "ios")]
    pub fn schedule_ios_layer_attach(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: OverlayPlayIntent,
    ) -> Result<IosLayerAttachOutcome> {
        let ios_intent = play_intent.clone_for_async();
        let work_generation = self.session.overlay_generation().load(Ordering::SeqCst);
        self.session.request_attach(
            shell,
            self.stored.clone(),
            self.clone_for_switch(),
            ios_intent,
            "load/play",
            work_generation,
            self.ios_layer_bus_slot(),
        )
    }

    /// 在 Gst 线程尝试附着 iOS 层；返回是否已调度或已绑定 / Tries iOS layer attach on Gst thread; returns whether scheduled or already bound.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 管线壳层 / pipeline shell
    /// - `play_intent` — 播放意图 / play intent
    ///
    /// # 返回值 / Returns
    /// - `Ok(true)` 已调度附着；`Ok(false)` 层未就绪；已绑定时返回当前绑定状态 /
    ///   `Ok(true)` when attach scheduled; `Ok(false)` when layer not ready; bound state when skipped
    #[cfg(target_os = "ios")]
    pub fn try_attach_ios_layer_on_gst(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: OverlayPlayIntent,
    ) -> Result<bool> {
        match self.schedule_ios_layer_attach(shell, play_intent)? {
            IosLayerAttachOutcome::Scheduled => Ok(true),
            IosLayerAttachOutcome::LayerNotReady => Ok(false),
            IosLayerAttachOutcome::Skipped => Ok(self.session.is_bound()),
        }
    }

    /// 管线壳层重建后使 overlay 状态失效 / Invalidates overlay state after pipeline shell rebuild.
    ///
    /// iOS 会先在主线程分离旧 sink 的 CALayer，再递增 generation。
    pub fn mark_shell_rebuilt(&self) {
        #[cfg(target_os = "ios")]
        self.detach_ios_sink_layers();
        self.session.mark_shell_rebuilt();
    }

    /// 在壳层拆除前，于主线程从宿主视图移除旧 sink 的 CALayer /
    /// Removes the previous sink's CALayer from the host view on the main thread
    /// before the shell (and its sink) is torn down, so a stale display layer
    /// with an in-flight data-request block cannot outlive the freed sink.
    #[cfg(target_os = "ios")]
    fn detach_ios_sink_layers(&self) {
        if let Some(host) = *self.stored.lock() {
            crate::platform::ios::detach_sink_layers_on_main_thread(host);
        }
    }

    /// 媒体变更（URI 重载、资源切换）时清除 overlay 状态 / Clears overlay state on every media change (URI reload, asset swap).
    #[cfg(target_os = "ios")]
    pub fn mark_media_changed(&self) {
        self.detach_ios_sink_layers();
        self.session.bump_overlay_generation();
        self.session.reset_for_media_change();
    }

    /// 取消进行中的 iOS overlay 工作（分离层 + 递增 generation）/ Cancels in-flight iOS overlay work (detach layers + bump generation).
    #[cfg(target_os = "ios")]
    pub fn cancel_ios_overlay_work(&self) {
        self.detach_ios_sink_layers();
        self.session.bump_overlay_generation();
        self.session.reset_for_shell_rebuild();
    }

    /// 在 overlay 绑定后通过 idle 路径恢复 iOS 播放 / Resumes iOS playback via idle path after overlay bind.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 管线壳层 / pipeline shell
    /// - `play_intent` — 恢复播放意图 / resume play intent
    #[cfg(target_os = "ios")]
    pub fn resume_ios_play(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: OverlayPlayIntent,
    ) {
        self.session.schedule_apply(self.session.idle_work(
            shell,
            self.stored.clone(),
            self.clone_for_switch(),
            play_intent,
            self.ios_layer_bus_slot(),
        ));
    }

    /// 缓存布局/report 的渲染矩形尺寸 / Caches layout/reported render rectangle dimensions.
    ///
    /// # 参数 / Parameters
    /// - `width` — 像素宽度 / pixel width
    /// - `height` — 像素高度 / pixel height
    pub fn set_cached_dimensions(&self, width: i32, height: i32) {
        self.session.set_cached_dimensions(width, height);
    }

    /// 返回缓存的渲染尺寸 / Returns cached render dimensions.
    ///
    /// # 返回值 / Returns
    /// - `(width, height)` 像素对 / `(width, height)` in pixels
    pub fn cached_dimensions(&self) -> (i32, i32) {
        self.session.cached_dimensions()
    }

    /// 缓存或清除原生宿主句柄 / Caches or clears the native host handle.
    ///
    /// # 参数 / Parameters
    /// - `handle` — 原生指针/句柄；`0` 清除缓存 / native pointer/handle; `0` clears cache
    pub fn cache_handle(&self, handle: usize) {
        if handle == 0 {
            self.stored.lock().take();
        } else {
            *self.stored.lock() = Some(handle);
        }
    }

    /// 返回 session 中的 overlay sink 共享槽（iOS）/ Returns the overlay sink shared slot in the session (iOS).
    #[cfg(target_os = "ios")]
    pub fn overlay_sink_slot(&self) -> Option<&Arc<Mutex<gst::Element>>> {
        self.session.overlay_sink_slot()
    }

    /// Texture/桌面路径无需预缓存 NSView；恒为就绪 / No NSView pre-cache needed on texture/desktop paths.
    pub fn ensure_overlay_ready(&self) -> Result<()> {
        Ok(())
    }

    /// 在新 video_sink 上重新绑定已缓存 overlay / Rebinds cached overlay onto a new video_sink.
    ///
    /// # 参数 / Parameters
    /// - `shell` — 已安装新 sink 的壳层 / shell with new sink installed
    ///
    /// # 返回值 / Returns
    /// - `Ok(())` 成功或无句柄 no-op / `Ok(())` on success or no-op without handle
    pub fn rebind_cached_overlay(&self, shell: &PipelineShell) -> Result<()> {
        self.session
            .rebind_cached_overlay(shell, self.stored.clone())
    }

    /// iOS：应用 GStreamer overlay（尺寸变更或首次附着）/ iOS: applies GStreamer overlay (resize or first attach).
    #[cfg(target_os = "ios")]
    pub fn apply_ios_overlay_gstreamer(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        self.session.apply_gstreamer(
            shell,
            self.stored.clone(),
            self.clone_for_switch(),
            width,
            height,
            play_intent,
        )
    }

    /// iOS：缓存宿主 `UIView` 句柄与尺寸（Gst 附着由 session 调度）/ iOS: caches host `UIView` handle and dimensions (Gst attach scheduled by session).
    #[cfg(target_os = "ios")]
    pub fn notify_ios_overlay(&self, handle: i64, width: i32, height: i32) -> Result<()> {
        self.session
            .cache_notify(&self.stored, handle, width, height)
    }

    /// Android：JNI Surface 回调入口 — 缓存并调度 Gst 绑定 / Android: JNI Surface callback entry — cache and schedule Gst bind.
    #[cfg(target_os = "android")]
    pub fn notify_android_surface(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        handle: i64,
        width: i32,
        height: i32,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        self.session.notify_surface_with_shell(
            self.stored.clone(),
            handle,
            width,
            height,
            shell,
            self.clone_for_switch(),
            play_intent,
        )
    }

    /// 桌面：在 Gst 线程设置窗口句柄并可能恢复播放 / Desktop: sets window handle on Gst thread and may resume playback.
    #[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
    pub fn set_window_handle_on_gst(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        window_handle: i64,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        {
            let guard = shell.lock();
            self.session
                .apply_window_handle(&guard, &self.stored, window_handle)?;
        }
        if window_handle != 0 {
            crate::playback::play_resume::maybe_resume_after_overlay_bind(
                shell,
                &play_intent.replay,
                &play_intent.swap,
                self,
            )?;
        }
        Ok(())
    }

    /// 桌面：在 Gst 线程调度 overlay 渲染矩形同步 / Desktop: schedules overlay render-rectangle sync on Gst thread.
    #[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
    pub fn schedule_overlay_rectangle_sync(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        self.session
            .schedule_rectangle_sync(self.stored.clone(), shell, width, height);
    }

    /// 移动端：缓存尺寸并在 Android 上调度 overlay 刷新 / Mobile: caches dimensions and schedules overlay refresh on Android.
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub fn schedule_mobile_overlay_rectangle_sync(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        self.set_cached_dimensions(width, height);
        #[cfg(target_os = "ios")]
        {
            let _ = shell;
        }
        #[cfg(target_os = "android")]
        {
            let stored = self.stored.clone();
            spawn_on_gst_thread(move || {
                let guard = shell.lock();
                let Some(handle) = *stored.lock() else {
                    return;
                };
                if let Err(e) =
                    refresh_mobile_overlay_on_gst(&guard, handle, width, height, "surface resize")
                {
                    crate::diag::logcat_error(&format!("mobile overlay resize: {e:#}"));
                }
            });
        }
    }

    /// 克隆用于管线切换的 surface（共享句柄槽与 session）/ Clones the surface for pipeline switch (shared handle slot and session).
    ///
    /// # 返回值 / Returns
    /// - 与当前实例共享 `stored` 与 session 状态的副本 / copy sharing `stored` and session state
    pub fn clone_for_switch(&self) -> Self {
        Self {
            stored: self.stored.clone(),
            #[cfg(target_os = "android")]
            session: self.session.clone(),
            #[cfg(target_os = "ios")]
            session: self.session.clone(),
            #[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
            session: self.session.clone(),
        }
    }
}
