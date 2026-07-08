use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

use crate::gst_runtime::spawn_on_gst_thread;
#[cfg(target_os = "android")]
use crate::playback::overlay::{
    cache_android_native_window, default_scheduler, refresh_mobile_overlay_on_gst,
    AndroidOverlayState,
};
#[cfg(target_os = "ios")]
use crate::playback::overlay::ios_session::IosOverlaySession;
#[cfg(target_os = "ios")]
use crate::playback::overlay::IosLayerBackend;
#[cfg(target_os = "macos")]
use crate::playback::overlay::MacosOverlayBackend;
use crate::playback::overlay::VideoOverlayBackend;
#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
use crate::playback::overlay::{apply_overlay_handle, DesktopOverlayBackend};
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
#[cfg(target_os = "ios")]
use crate::video::ios_layer::IosLayerAttachOutcome;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use crate::playback::overlay::assign_overlay_sink;

/// Nested iOS overlay state on [`VideoSurface`].
#[cfg(target_os = "ios")]
pub struct IosOverlayState {
    pub session: IosOverlaySession,
    last_applied_handle: Arc<AtomicUsize>,
    pub ios_layer_bus_slot: Arc<Mutex<Option<IosLayerBackend>>>,
    overlay_sink: Option<Arc<Mutex<gst::Element>>>,
    last_width: Arc<AtomicI32>,
    last_height: Arc<AtomicI32>,
}

#[cfg(target_os = "ios")]
impl IosOverlayState {
    fn new(running: Arc<AtomicBool>) -> Self {
        let overlay_bound = Arc::new(AtomicBool::new(false));
        let last_applied_handle = Arc::new(AtomicUsize::new(0));
        let session = IosOverlaySession::new(
            overlay_bound,
            last_applied_handle.clone(),
            running,
            Arc::new(std::sync::atomic::AtomicU64::new(0)),
        );
        Self {
            session,
            last_applied_handle,
            ios_layer_bus_slot: Arc::new(Mutex::new(None)),
            overlay_sink: None,
            last_width: Arc::new(AtomicI32::new(0)),
            last_height: Arc::new(AtomicI32::new(0)),
        }
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
}

/// Cached native overlay handle and platform-specific bind helpers.
pub struct VideoSurface {
    stored: Arc<Mutex<Option<usize>>>,
    #[cfg(target_os = "android")]
    android: AndroidOverlayState,
    #[cfg(target_os = "ios")]
    ios: IosOverlayState,
    #[cfg(target_os = "macos")]
    overlay_sink: Option<Arc<Mutex<gst::Element>>>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl VideoOverlayBackend for VideoSurface {
    fn stored_handle(&self) -> &Mutex<Option<usize>> {
        self.stored.as_ref()
    }
}

impl VideoSurface {
    pub fn new(stored: Arc<Mutex<Option<usize>>>) -> Self {
        Self {
            stored,
            #[cfg(target_os = "android")]
            android: AndroidOverlayState::new(),
            #[cfg(target_os = "ios")]
            ios: IosOverlayState::new(Arc::new(AtomicBool::new(true))),
            #[cfg(target_os = "macos")]
            overlay_sink: None,
        }
    }

    #[cfg(target_os = "ios")]
    pub fn ios_layer_bus_slot(&self) -> Arc<Mutex<Option<IosLayerBackend>>> {
        self.ios.ios_layer_bus_slot.clone()
    }

    #[cfg(target_os = "ios")]
    pub fn register_ios_layer_backend(&self, backend: IosLayerBackend) {
        *self.ios.ios_layer_bus_slot.lock() = Some(backend);
    }

    #[cfg(target_os = "ios")]
    pub fn ios_session(&self) -> IosOverlaySession {
        self.ios.session.clone()
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn with_overlay_sink_slot(
        stored: Arc<Mutex<Option<usize>>>,
        overlay_sink: Arc<Mutex<gst::Element>>,
        #[cfg(target_os = "ios")] running: Arc<AtomicBool>,
    ) -> Self {
        #[cfg(target_os = "ios")]
        let mut ios = IosOverlayState::new(running);
        #[cfg(target_os = "ios")]
        {
            ios.overlay_sink = Some(overlay_sink.clone());
        }
        Self {
            stored,
            #[cfg(target_os = "android")]
            android: AndroidOverlayState::new(),
            #[cfg(target_os = "ios")]
            ios,
            #[cfg(target_os = "macos")]
            overlay_sink: Some(overlay_sink),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn with_macos_overlay_sink(
        stored: Arc<Mutex<Option<usize>>>,
        overlay_sink: Arc<Mutex<gst::Element>>,
    ) -> Self {
        Self::with_overlay_sink_slot(stored, overlay_sink)
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn set_overlay_sink_slot(&mut self, element: gst::Element) {
        #[cfg(target_os = "ios")]
        {
            match &self.ios.overlay_sink {
                Some(slot) => *slot.lock() = element,
                None => self.ios.overlay_sink = Some(Arc::new(Mutex::new(element))),
            }
        }
        #[cfg(target_os = "macos")]
        {
            match &self.overlay_sink {
                Some(slot) => *slot.lock() = element,
                None => self.overlay_sink = Some(Arc::new(Mutex::new(element))),
            }
        }
    }

    #[cfg(target_os = "macos")]
    pub fn set_macos_overlay_sink(&mut self, element: gst::Element) {
        self.set_overlay_sink_slot(element);
    }

    pub fn stored_handle(&self) -> Arc<Mutex<Option<usize>>> {
        self.stored.clone()
    }

    pub fn has_cached_handle(&self) -> bool {
        self.stored.lock().is_some()
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub fn is_overlay_bound_on_gst(&self) -> bool {
        #[cfg(target_os = "android")]
        {
            return self.android.is_overlay_bound_on_gst();
        }
        #[cfg(target_os = "ios")]
        {
            return self.ios.session.is_bound();
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn is_overlay_bound_on_gst(&self) -> bool {
        self.has_cached_handle()
    }

    pub fn overlay_ready_for_preroll(&self) -> bool {
        #[cfg(target_os = "android")]
        {
            return self
                .android
                .overlay_ready_for_preroll(self.has_cached_handle());
        }
        #[cfg(target_os = "ios")]
        {
            return self.has_cached_handle();
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            VideoOverlayBackend::overlay_ready_for_preroll(self)
        }
    }

    #[cfg(target_os = "ios")]
    pub fn schedule_ios_layer_attach(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: OverlayPlayIntent,
    ) -> Result<IosLayerAttachOutcome> {
        let ios_intent = play_intent.clone_for_async();
        let work_generation = self.ios.session.overlay_generation().load(Ordering::SeqCst);
        self.ios.session.request_attach(
            shell,
            self.stored.clone(),
            self.clone_for_switch(),
            ios_intent,
            "load/play",
            work_generation,
            self.ios_layer_bus_slot(),
        )
    }

    #[cfg(target_os = "ios")]
    pub fn try_attach_ios_layer_on_gst(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: OverlayPlayIntent,
    ) -> Result<bool> {
        match self.schedule_ios_layer_attach(shell, play_intent)? {
            IosLayerAttachOutcome::Scheduled => Ok(true),
            IosLayerAttachOutcome::LayerNotReady => Ok(false),
            IosLayerAttachOutcome::Skipped => Ok(self.ios.session.is_bound()),
        }
    }

    pub fn mark_shell_rebuilt(&self) {
        #[cfg(target_os = "android")]
        self.android.mark_shell_rebuilt();
        #[cfg(target_os = "ios")]
        {
            self.ios.session.bump_overlay_generation();
            self.ios.session.reset_for_shell_rebuild();
        }
    }

    #[cfg(target_os = "ios")]
    pub fn mark_media_changed(&self) {
        self.ios.session.bump_overlay_generation();
        self.ios.session.reset_for_media_change();
    }

    #[cfg(target_os = "ios")]
    pub fn cancel_ios_overlay_work(&self) {
        self.ios.session.bump_overlay_generation();
        self.ios.session.reset_for_shell_rebuild();
    }

    #[cfg(target_os = "ios")]
    pub fn resume_ios_play(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: OverlayPlayIntent,
    ) {
        self.ios.session.schedule_apply(self.ios.session.idle_work(
            shell,
            self.stored.clone(),
            self.clone_for_switch(),
            play_intent,
            self.ios_layer_bus_slot(),
        ));
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub fn set_cached_dimensions(&self, width: i32, height: i32) {
        #[cfg(target_os = "android")]
        self.android.set_cached_dimensions(width, height);
        #[cfg(target_os = "ios")]
        self.ios.set_cached_dimensions(width, height);
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub fn cached_dimensions(&self) -> (i32, i32) {
        #[cfg(target_os = "android")]
        {
            return self.android.cached_dimensions();
        }
        #[cfg(target_os = "ios")]
        {
            return self.ios.cached_dimensions();
        }
    }

    pub fn cache_handle(&self, handle: usize) {
        if handle == 0 {
            self.stored.lock().take();
        } else {
            *self.stored.lock() = Some(handle);
        }
    }

    #[cfg(target_os = "macos")]
    pub fn cache_macos_handle(&self, view_ptr: i64) {
        if view_ptr == 0 {
            self.cache_handle(0);
        } else {
            self.cache_handle(view_ptr as usize);
        }
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn overlay_sink_slot(&self) -> Option<&Arc<Mutex<gst::Element>>> {
        #[cfg(target_os = "ios")]
        {
            return self.ios.overlay_sink.as_ref();
        }
        #[cfg(target_os = "macos")]
        {
            return self.overlay_sink.as_ref();
        }
    }

    #[cfg(target_os = "macos")]
    pub fn macos_overlay_sink(&self) -> Option<&Arc<Mutex<gst::Element>>> {
        self.overlay_sink_slot()
    }

    #[cfg(target_os = "macos")]
    pub fn apply_macos_overlay_gstreamer(&self, width: i32, height: i32) -> Result<()> {
        let Some(slot) = self.overlay_sink.as_ref() else {
            return Ok(());
        };
        MacosOverlayBackend::apply_gstreamer(&self.stored, &slot.lock(), width, height)
    }

    #[cfg(target_os = "macos")]
    pub fn ensure_overlay_ready(&self) -> Result<()> {
        MacosOverlayBackend::ensure_overlay_ready(&self.stored)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn ensure_overlay_ready(&self) -> Result<()> {
        Ok(())
    }

    pub fn rebind_cached_overlay(&self, shell: &PipelineShell) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let _ = (self, shell);
            return Ok(());
        }
        #[cfg(target_os = "android")]
        {
            if let Some(handle) = *self.stored.lock() {
                let (width, height) = self.android.cached_dimensions();
                refresh_mobile_overlay_on_gst(shell, handle, width, height, "rebind")?;
                self.android.session.set_bound(true);
                crate::diag::logcat_info("gst: overlay rebound on new video_sink");
            }
            return Ok(());
        }
        #[cfg(target_os = "ios")]
        {
            let _ = (self, shell);
            return Ok(());
        }
        #[cfg(all(
            not(target_os = "macos"),
            not(target_os = "android"),
            not(target_os = "ios")
        ))]
        {
            DesktopOverlayBackend::rebind_cached_overlay(&self.stored, shell)
        }
    }

    #[cfg(target_os = "ios")]
    fn cache_ios_overlay(&self, handle: i64, width: i32, height: i32) {
        if handle == 0 {
            self.ios.session.reset_for_host_change();
            self.cache_handle(0);
            return;
        }
        let new_handle = handle as usize;
        let host_changed = match *self.stored.lock() {
            Some(h) if h != 0 => h != new_handle,
            _ => false,
        };
        self.ios.set_cached_dimensions(width, height);
        if host_changed {
            self.ios.session.reset_for_host_change();
        }
        self.cache_handle(new_handle);
    }

    #[cfg(target_os = "ios")]
    pub fn apply_ios_overlay_gstreamer(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let (prev_w, prev_h) = self.ios.cached_dimensions();
        self.ios.set_cached_dimensions(width, height);
        let host_view = *self.stored.lock();
        let Some(host_view) = host_view else {
            return Ok(());
        };

        let last_applied = self.ios.last_applied_handle.load(Ordering::SeqCst);
        if last_applied != 0 && last_applied != host_view {
            self.ios.session.reset_for_host_change();
        }

        if self.ios.session.is_bound()
            && self.ios.last_applied_handle.load(Ordering::SeqCst) == host_view
        {
            let dimensions_changed = prev_w != width || prev_h != height;
            if dimensions_changed || play_intent.replay.desired_playing.load(Ordering::SeqCst) {
                let session = self.ios.session.clone();
                let stored = self.stored.clone();
                let surface_for_work = self.clone_for_switch();
                let running = play_intent.replay.running.clone();
                let work_generation = session.overlay_generation().load(Ordering::SeqCst);
                let ios_intent = play_intent.clone_for_async();
                let ios_slot = self.ios_layer_bus_slot();
                spawn_on_gst_thread(move || {
                    if !running.load(Ordering::SeqCst)
                        || work_generation != session.overlay_generation().load(Ordering::SeqCst)
                    {
                        return;
                    }
                    if dimensions_changed {
                        let sink = {
                            let guard = shell.lock();
                            guard.video_sink.clone()
                        };
                        if let Ok(layer) = crate::video::ios_layer::read_sink_layer(&sink) {
                            if !crate::platform_view_ios::attach_layer_on_main_thread_sync(
                                host_view, layer,
                            ) {
                                crate::video::ios_layer::release_sink_layer(layer);
                            }
                        }
                    }
                    if !running.load(Ordering::SeqCst)
                        || work_generation != session.overlay_generation().load(Ordering::SeqCst)
                    {
                        return;
                    }
                    session.drain_pending_play(session.idle_work(
                        shell,
                        stored,
                        surface_for_work,
                        ios_intent,
                        ios_slot,
                    ));
                });
            }
            return Ok(());
        }

        let session = self.ios.session.clone();
        let stored = self.stored.clone();
        let surface_for_attach = self.clone_for_switch();
        let running = play_intent.replay.running.clone();
        let work_generation = session.overlay_generation().load(Ordering::SeqCst);
        let ios_intent = play_intent.clone_for_async();
        let ios_slot = self.ios_layer_bus_slot();
        spawn_on_gst_thread(move || {
            if !running.load(Ordering::SeqCst)
                || work_generation != session.overlay_generation().load(Ordering::SeqCst)
            {
                return;
            }
            let _ = session.request_attach(
                shell,
                stored,
                surface_for_attach,
                ios_intent,
                "Swift apply",
                work_generation,
                ios_slot,
            );
        });
        Ok(())
    }

    #[cfg(target_os = "ios")]
    pub fn notify_ios_overlay(&self, handle: i64, width: i32, height: i32) -> Result<()> {
        self.cache_ios_overlay(handle, width, height);
        Ok(())
    }

    #[cfg(target_os = "android")]
    pub fn notify_android_surface(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        handle: i64,
        width: i32,
        height: i32,
        play_intent: OverlayPlayIntent,
    ) -> Result<()> {
        let scheduler = default_scheduler();
        if handle == 0 {
            let work_generation = self.android.session.work_generation();
            self.android
                .session
                .cache_surface_notify(&self.stored, 0, 0, 0, &self.android)?;
            self.android
                .session
                .schedule_clear_overlay(shell, work_generation, &scheduler);
            return Ok(());
        }
        self.android
            .session
            .cache_surface_notify(&self.stored, handle, width, height, &self.android)?;
        let (w, h) = self.android.cached_dimensions();
        self.android.session.schedule_apply_after_bind(
            shell,
            self.stored.clone(),
            w,
            h,
            self.clone_for_switch(),
            play_intent,
            &scheduler,
        );
        Ok(())
    }

    #[cfg(all(
        not(target_os = "android"),
        not(target_os = "macos"),
        not(target_os = "ios")
    ))]
    pub fn set_window_handle_on_gst(
        &self,
        shell: &mut PipelineShell,
        window_handle: i64,
    ) -> Result<()> {
        let handle = window_handle as usize;
        apply_overlay_handle(&shell.video_sink, handle, &self.stored)
    }

    #[cfg(not(any(target_os = "android", target_os = "macos", target_os = "ios")))]
    pub fn schedule_overlay_rectangle_sync(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        DesktopOverlayBackend::schedule_rectangle_sync(self.stored.clone(), shell, width, height);
    }

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
            return;
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

    pub fn clone_for_switch(&self) -> Self {
        Self {
            stored: self.stored.clone(),
            #[cfg(target_os = "android")]
            android: self.android.clone_for_switch(),
            #[cfg(target_os = "ios")]
            ios: IosOverlayState {
                session: self.ios.session.clone(),
                last_applied_handle: self.ios.last_applied_handle.clone(),
                ios_layer_bus_slot: self.ios.ios_layer_bus_slot.clone(),
                overlay_sink: self.ios.overlay_sink.clone(),
                last_width: self.ios.last_width.clone(),
                last_height: self.ios.last_height.clone(),
            },
            #[cfg(target_os = "macos")]
            overlay_sink: self.overlay_sink.clone(),
        }
    }
}
