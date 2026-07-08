use std::sync::Arc;

#[cfg(any(target_os = "ios", target_os = "macos"))]
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
#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
use crate::playback::overlay::DesktopOverlaySession;
#[cfg(target_os = "ios")]
use crate::playback::overlay::IosLayerBackend;
#[cfg(target_os = "ios")]
use crate::playback::overlay::IosOverlaySession;
use crate::playback::overlay::OverlaySession;
use crate::playback::overlay::VideoOverlayBackend;
#[cfg(target_os = "macos")]
use crate::playback::overlay::MacosOverlaySession;
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    all(
        not(target_os = "android"),
        not(target_os = "macos"),
        not(target_os = "ios")
    )
))]
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;

/// Cached native overlay handle — thin delegate to platform [`OverlaySession`].
pub struct VideoSurface {
    stored: Arc<Mutex<Option<usize>>>,
    #[cfg(target_os = "android")]
    session: AndroidOverlaySession,
    #[cfg(target_os = "ios")]
    session: IosOverlaySession,
    #[cfg(target_os = "macos")]
    session: MacosOverlaySession,
    #[cfg(all(
        not(target_os = "android"),
        not(target_os = "macos"),
        not(target_os = "ios")
    ))]
    session: DesktopOverlaySession,
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
            session: AndroidOverlaySession::new(),
            #[cfg(target_os = "ios")]
            session: IosOverlaySession::new_with_running(Arc::new(AtomicBool::new(true))),
            #[cfg(target_os = "macos")]
            session: MacosOverlaySession::new(),
            #[cfg(all(
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            session: DesktopOverlaySession::new(),
        }
    }

    #[cfg(target_os = "ios")]
    pub fn wire_ios_replay_running(&mut self, running: Arc<AtomicBool>) {
        self.session.wire_running(running);
    }

    pub fn overlay_session(&self) -> &dyn OverlaySession {
        &self.session
    }

    #[cfg(target_os = "ios")]
    pub fn ios_session(&self) -> IosOverlaySession {
        self.session.clone()
    }

    #[cfg(target_os = "ios")]
    pub fn ios_layer_bus_slot(&self) -> Arc<Mutex<Option<IosLayerBackend>>> {
        self.session.ios_layer_bus_slot.clone()
    }

    #[cfg(target_os = "ios")]
    pub fn register_ios_layer_backend(&self, backend: IosLayerBackend) {
        self.session.register_ios_layer_backend(backend);
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn with_overlay_sink_slot(
        stored: Arc<Mutex<Option<usize>>>,
        overlay_sink: Arc<Mutex<gst::Element>>,
        #[cfg(target_os = "ios")] running: Arc<AtomicBool>,
    ) -> Self {
        #[cfg(target_os = "ios")]
        let mut session = IosOverlaySession::new_with_running(running);
        #[cfg(target_os = "ios")]
        session.set_overlay_sink(overlay_sink.lock().clone());
        #[cfg(target_os = "macos")]
        let mut session = MacosOverlaySession::new();
        #[cfg(target_os = "macos")]
        session.set_overlay_sink(overlay_sink.lock().clone());
        Self {
            stored,
            #[cfg(target_os = "android")]
            session: AndroidOverlaySession::new(),
            #[cfg(target_os = "ios")]
            session,
            #[cfg(target_os = "macos")]
            session,
            #[cfg(all(
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            session: DesktopOverlaySession::new(),
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
        self.session.set_overlay_sink(element);
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

    pub fn is_overlay_bound_on_gst(&self) -> bool {
        self.session.is_bound()
    }

    pub fn overlay_ready_for_preroll(&self) -> bool {
        self.session
            .overlay_ready_for_preroll(self.has_cached_handle())
    }

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

    pub fn mark_shell_rebuilt(&self) {
        #[cfg(target_os = "ios")]
        self.detach_ios_sink_layers();
        self.session.mark_shell_rebuilt();
    }

    /// Removes the previous sink's CALayer from the host view on the main thread
    /// before the shell (and its sink) is torn down, so a stale display layer
    /// with an in-flight data-request block cannot outlive the freed sink.
    #[cfg(target_os = "ios")]
    fn detach_ios_sink_layers(&self) {
        if let Some(host) = *self.stored.lock() {
            crate::platform::ios::detach_sink_layers_on_main_thread(host);
        }
    }

    #[cfg(target_os = "ios")]
    pub fn mark_media_changed(&self) {
        self.detach_ios_sink_layers();
        self.session.bump_overlay_generation();
        self.session.reset_for_media_change();
    }

    #[cfg(target_os = "ios")]
    pub fn cancel_ios_overlay_work(&self) {
        self.detach_ios_sink_layers();
        self.session.bump_overlay_generation();
        self.session.reset_for_shell_rebuild();
    }

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

    pub fn set_cached_dimensions(&self, width: i32, height: i32) {
        self.session.set_cached_dimensions(width, height);
    }

    pub fn cached_dimensions(&self) -> (i32, i32) {
        self.session.cached_dimensions()
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
        let _ = self.session.cache_notify(&self.stored, view_ptr, 0, 0);
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn overlay_sink_slot(&self) -> Option<&Arc<Mutex<gst::Element>>> {
        self.session.overlay_sink_slot()
    }

    #[cfg(target_os = "macos")]
    pub fn macos_overlay_sink(&self) -> Option<&Arc<Mutex<gst::Element>>> {
        self.overlay_sink_slot()
    }

    #[cfg(target_os = "macos")]
    pub fn apply_macos_overlay_gstreamer(
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

    #[cfg(target_os = "macos")]
    pub fn ensure_overlay_ready(&self) -> Result<()> {
        self.session.ensure_overlay_ready(&self.stored)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn ensure_overlay_ready(&self) -> Result<()> {
        Ok(())
    }

    pub fn rebind_cached_overlay(&self, shell: &PipelineShell) -> Result<()> {
        self.session
            .rebind_cached_overlay(shell, self.stored.clone())
    }

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

    #[cfg(target_os = "ios")]
    pub fn notify_ios_overlay(&self, handle: i64, width: i32, height: i32) -> Result<()> {
        self.session
            .cache_notify(&self.stored, handle, width, height)
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

    #[cfg(all(
        not(target_os = "android"),
        not(target_os = "macos"),
        not(target_os = "ios")
    ))]
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

    #[cfg(not(any(target_os = "android", target_os = "macos", target_os = "ios")))]
    pub fn schedule_overlay_rectangle_sync(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        self.session
            .schedule_rectangle_sync(self.stored.clone(), shell, width, height);
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
            session: self.session.clone(),
            #[cfg(target_os = "ios")]
            session: self.session.clone(),
            #[cfg(target_os = "macos")]
            session: self.session.clone(),
            #[cfg(all(
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            session: self.session.clone(),
        }
    }
}
