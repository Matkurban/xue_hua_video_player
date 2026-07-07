use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use parking_lot::Mutex;

use crate::gst_runtime::spawn_on_gst_thread;
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
#[cfg(target_os = "android")]
use crate::playback::overlay::{
    cache_android_native_window, refresh_mobile_overlay_on_gst, schedule_mobile_overlay_apply,
};
use crate::playback::replay::OverlayPlayIntent;
use crate::playback::shell::PipelineShell;
use crate::video::clear_overlay_window_handle;
#[cfg(target_os = "ios")]
use crate::video::ios_layer::IosLayerAttachOutcome;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use crate::playback::overlay::assign_overlay_sink;

#[cfg(target_os = "android")]
pub use crate::playback::overlay::refresh_mobile_overlay_on_gst;

/// Cached native overlay handle and platform-specific bind helpers.
pub struct VideoSurface {
    stored: Arc<Mutex<Option<usize>>>,
    #[cfg(any(target_os = "android", target_os = "ios"))]
    overlay_bound: Arc<AtomicBool>,
    #[cfg(any(target_os = "android", target_os = "ios"))]
    last_width: Arc<AtomicI32>,
    #[cfg(any(target_os = "android", target_os = "ios"))]
    last_height: Arc<AtomicI32>,
    #[cfg(target_os = "ios")]
    last_applied_handle: Arc<AtomicUsize>,
    #[cfg(target_os = "ios")]
    ios_session: IosOverlaySession,
    #[cfg(target_os = "ios")]
    ios_layer_bus_slot: Option<Arc<Mutex<Option<crate::playback::overlay::IosLayerBackend>>>>,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    overlay_sink: Option<Arc<Mutex<gst::Element>>>,
}

impl VideoOverlayBackend for VideoSurface {
    fn stored_handle(&self) -> &Mutex<Option<usize>> {
        self.stored.as_ref()
    }

    #[cfg(target_os = "android")]
    fn overlay_ready_for_preroll(&self) -> bool {
        self.has_cached_handle() && self.is_overlay_bound_on_gst()
    }
}

impl VideoSurface {
    pub fn new(stored: Arc<Mutex<Option<usize>>>) -> Self {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let overlay_bound = Arc::new(AtomicBool::new(false));
        #[cfg(target_os = "ios")]
        let last_applied_handle = Arc::new(AtomicUsize::new(0));
        #[cfg(target_os = "ios")]
        let ios_session = IosOverlaySession::new(
            overlay_bound.clone(),
            last_applied_handle.clone(),
            Arc::new(AtomicBool::new(true)),
            Arc::new(std::sync::atomic::AtomicU64::new(0)),
        );
        Self {
            stored,
            #[cfg(any(target_os = "android", target_os = "ios"))]
            overlay_bound,
            #[cfg(any(target_os = "android", target_os = "ios"))]
            last_width: Arc::new(AtomicI32::new(0)),
            #[cfg(any(target_os = "android", target_os = "ios"))]
            last_height: Arc::new(AtomicI32::new(0)),
            #[cfg(target_os = "ios")]
            last_applied_handle,
            #[cfg(target_os = "ios")]
            ios_session,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            overlay_sink: None,
            #[cfg(target_os = "ios")]
            ios_layer_bus_slot: None,
        }
    }

    #[cfg(target_os = "ios")]
    pub fn set_ios_layer_bus_slot(&mut self, slot: Arc<Mutex<Option<IosLayerBackend>>>) {
        self.ios_layer_bus_slot = Some(slot);
    }

    #[cfg(target_os = "ios")]
    fn ios_layer_bus_slot(&self) -> Arc<Mutex<Option<IosLayerBackend>>> {
        self.ios_layer_bus_slot
            .as_ref()
            .expect("ios layer bus slot not wired")
            .clone()
    }

    #[cfg(target_os = "ios")]
    pub fn ios_session(&self) -> IosOverlaySession {
        self.ios_session.clone()
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn with_overlay_sink_slot(
        stored: Arc<Mutex<Option<usize>>>,
        overlay_sink: Arc<Mutex<gst::Element>>,
        #[cfg(target_os = "ios")] running: Arc<AtomicBool>,
    ) -> Self {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let overlay_bound = Arc::new(AtomicBool::new(false));
        #[cfg(target_os = "ios")]
        let last_applied_handle = Arc::new(AtomicUsize::new(0));
        #[cfg(target_os = "ios")]
        let ios_session = IosOverlaySession::new(
            overlay_bound.clone(),
            last_applied_handle.clone(),
            running,
            Arc::new(std::sync::atomic::AtomicU64::new(0)),
        );
        Self {
            stored,
            #[cfg(any(target_os = "android", target_os = "ios"))]
            overlay_bound,
            #[cfg(any(target_os = "android", target_os = "ios"))]
            last_width: Arc::new(AtomicI32::new(0)),
            #[cfg(any(target_os = "android", target_os = "ios"))]
            last_height: Arc::new(AtomicI32::new(0)),
            #[cfg(target_os = "ios")]
            last_applied_handle,
            #[cfg(target_os = "ios")]
            ios_session,
            overlay_sink: Some(overlay_sink),
            #[cfg(target_os = "ios")]
            ios_layer_bus_slot: None,
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
        match &self.overlay_sink {
            Some(slot) => *slot.lock() = element,
            None => {
                self.overlay_sink = Some(Arc::new(Mutex::new(element)));
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
        VideoOverlayBackend::has_cached_handle(self)
    }

    /// True once the platform overlay is bound on the Gst thread (Android: VideoOverlay; iOS: CALayer attached).
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub fn is_overlay_bound_on_gst(&self) -> bool {
        self.overlay_bound.load(Ordering::SeqCst)
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn is_overlay_bound_on_gst(&self) -> bool {
        self.has_cached_handle()
    }

    /// True when a cached native handle exists (Android: also requires Gst-thread bind).
    pub fn overlay_ready_for_preroll(&self) -> bool {
        VideoOverlayBackend::overlay_ready_for_preroll(self)
    }

    /// Schedules async CALayer attach on the main thread (Tutorial 4 target_state flow).
    #[cfg(target_os = "ios")]
    pub fn schedule_ios_layer_attach(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: OverlayPlayIntent,
    ) -> Result<IosLayerAttachOutcome> {
        let ios_intent = play_intent.clone_for_async();
        let work_generation = self.ios_session.overlay_generation().load(Ordering::SeqCst);
        self.ios_session.request_attach(
            shell,
            self.stored.clone(),
            ios_intent,
            "load/play",
            work_generation,
            self.ios_layer_bus_slot(),
        )
    }

    /// Attaches `avsamplebufferlayersink` CALayer when preroll has exposed the layer property.
    #[cfg(target_os = "ios")]
    pub fn try_attach_ios_layer_on_gst(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: OverlayPlayIntent,
    ) -> Result<bool> {
        match self.schedule_ios_layer_attach(shell, play_intent)? {
            IosLayerAttachOutcome::Scheduled => Ok(true),
            IosLayerAttachOutcome::LayerNotReady => Ok(false),
            IosLayerAttachOutcome::Skipped => Ok(self.overlay_bound.load(Ordering::SeqCst)),
        }
    }

    /// Clears overlay-bound state when the pipeline shell's video sink is replaced.
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub fn mark_shell_rebuilt(&self) {
        #[cfg(target_os = "ios")]
        {
            self.ios_session.bump_overlay_generation();
            self.ios_session.reset_for_shell_rebuild();
        }
        #[cfg(not(target_os = "ios"))]
        self.overlay_bound.store(false, Ordering::SeqCst);
    }

    /// Clears iOS overlay bind state on every `load` (URI→URI reload, not only shell rebuild).
    #[cfg(target_os = "ios")]
    pub fn mark_media_changed(&self) {
        self.ios_session.bump_overlay_generation();
        self.ios_session.reset_for_media_change();
    }

    /// Invalidates queued iOS overlay idle work (dispose / teardown).
    #[cfg(target_os = "ios")]
    pub fn cancel_ios_overlay_work(&self) {
        self.ios_session.bump_overlay_generation();
        self.ios_session.reset_for_shell_rebuild();
    }

    /// Resumes PLAYING through [`IosOverlaySession`] when overlay is already bound.
    #[cfg(target_os = "ios")]
    pub fn resume_ios_play(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: OverlayPlayIntent,
    ) {
        let ios_intent = play_intent.clone_for_async();
        self.ios_session.drain_pending_play(
            shell,
            &ios_intent,
            self.ios_session.overlay_generation().load(Ordering::SeqCst),
            self.ios_layer_bus_slot(),
        );
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn mark_shell_rebuilt(&self) {}

    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub fn set_cached_dimensions(&self, width: i32, height: i32) {
        if width > 0 {
            self.last_width.store(width, Ordering::SeqCst);
        }
        if height > 0 {
            self.last_height.store(height, Ordering::SeqCst);
        }
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub fn cached_dimensions(&self) -> (i32, i32) {
        (
            self.last_width.load(Ordering::SeqCst),
            self.last_height.load(Ordering::SeqCst),
        )
    }

    pub fn cache_handle(&self, handle: usize) {
        VideoOverlayBackend::cache_handle(self, handle);
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
        self.overlay_sink.as_ref()
    }

    #[cfg(target_os = "macos")]
    pub fn macos_overlay_sink(&self) -> Option<&Arc<Mutex<gst::Element>>> {
        self.overlay_sink_slot()
    }

    /// Applies VideoOverlay on the **main thread** (macOS `osxvideosink`).
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
                let (width, height) = self.cached_dimensions();
                refresh_mobile_overlay_on_gst(shell, handle, width, height, "rebind")?;
                self.overlay_bound.store(true, Ordering::SeqCst);
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
    pub fn cache_ios_overlay(&self, handle: i64, width: i32, height: i32) {
        if handle == 0 {
            self.ios_session.reset_for_host_change();
            self.cache_handle(0);
            return;
        }
        let new_handle = handle as usize;
        let host_changed = match *self.stored.lock() {
            Some(h) if h != 0 => h != new_handle,
            _ => false,
        };
        self.set_cached_dimensions(width, height);
        if host_changed {
            self.ios_session.reset_for_host_change();
        }
        self.cache_handle(new_handle);
    }

    /// Attaches `avsamplebufferlayersink` CALayer and prerolls on `xhvp-gst` (Swift calls from main).
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
        let (prev_w, prev_h) = self.cached_dimensions();
        self.set_cached_dimensions(width, height);
        let host_view = *self.stored.lock();
        let Some(host_view) = host_view else {
            return Ok(());
        };

        let last_applied = self.last_applied_handle.load(Ordering::SeqCst);
        if last_applied != 0 && last_applied != host_view {
            self.ios_session.reset_for_host_change();
        }

        if self.ios_session.is_bound()
            && self.last_applied_handle.load(Ordering::SeqCst) == host_view
        {
            let dimensions_changed = prev_w != width || prev_h != height;
            if dimensions_changed
                || play_intent
                    .bundle
                    .replay
                    .desired_playing
                    .load(Ordering::SeqCst)
            {
                let session = self.ios_session.clone();
                let running = play_intent.bundle.replay.running.clone();
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
                    session.drain_pending_play(shell, &ios_intent, work_generation, ios_slot);
                });
            }
            return Ok(());
        }

        let session = self.ios_session.clone();
        let stored = self.stored.clone();
        let running = play_intent.bundle.replay.running.clone();
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
        if handle == 0 {
            self.overlay_bound.store(false, Ordering::SeqCst);
            cache_android_native_window(&self.stored, 0)?;
            let overlay_bound = self.overlay_bound.clone();
            spawn_on_gst_thread(move || {
                let guard = shell.lock();
                if let Err(e) = clear_overlay_window_handle(&guard.video_sink) {
                    log::warn!("android overlay clear: {e:#}");
                }
                overlay_bound.store(false, Ordering::SeqCst);
            });
            return Ok(());
        }
        self.set_cached_dimensions(width, height);
        self.overlay_bound.store(false, Ordering::SeqCst);
        cache_android_native_window(&self.stored, handle as usize)?;
        schedule_mobile_overlay_apply(
            shell,
            self.stored.clone(),
            self.overlay_bound.clone(),
            width,
            height,
            play_intent,
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

    /// Applies render rectangle + expose on the Gst thread (desktop).
    #[cfg(not(any(target_os = "android", target_os = "macos", target_os = "ios")))]
    pub fn schedule_overlay_rectangle_sync(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        DesktopOverlayBackend::schedule_rectangle_sync(self.stored.clone(), shell, width, height);
    }

    /// Re-applies VideoOverlay render rectangle + expose after surface resize (Android/iOS).
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

    /// Shares overlay state for [`super::switch::ShellTransition`].
    pub fn clone_for_switch(&self) -> Self {
        Self {
            stored: self.stored.clone(),
            #[cfg(any(target_os = "android", target_os = "ios"))]
            overlay_bound: self.overlay_bound.clone(),
            #[cfg(any(target_os = "android", target_os = "ios"))]
            last_width: self.last_width.clone(),
            #[cfg(any(target_os = "android", target_os = "ios"))]
            last_height: self.last_height.clone(),
            #[cfg(target_os = "ios")]
            last_applied_handle: self.last_applied_handle.clone(),
            #[cfg(target_os = "ios")]
            ios_session: self.ios_session.clone(),
            #[cfg(target_os = "ios")]
            ios_layer_bus_slot: self.ios_layer_bus_slot.clone(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            overlay_sink: self.overlay_sink.clone(),
        }
    }
}
