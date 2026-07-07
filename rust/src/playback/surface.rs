use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::gst_runtime::spawn_on_gst_thread;
use crate::playback::shell::PipelineShell;
use crate::playback::state::set_state_sync;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::playback::state::resume_or_replay_from_eos;
#[cfg(target_os = "ios")]
use crate::platform_view_ios::detach_sink_layers_from_host;
#[cfg(target_os = "ios")]
use crate::playback::ios_overlay::{IosIdleWork, IosOverlayPlayIntent, IosOverlaySession};
use crate::playback::switch::SwitchContext;
use crate::video::{
    clear_overlay_window_handle, expose_overlay, set_overlay_render_rectangle,
    set_overlay_window_handle,
};
#[cfg(target_os = "ios")]
use crate::video::ios_layer::IosLayerAttachOutcome;

/// Shared state for iOS layer attach retries on the Gst bus (`READY → PAUSED`).
#[cfg(target_os = "ios")]
pub struct IosLayerBusHook {
    pub shell: Arc<Mutex<PipelineShell>>,
    pub ios_session: IosOverlaySession,
    pub stored: Arc<Mutex<Option<usize>>>,
    pub overlay_sink: Arc<Mutex<gst::Element>>,
    pub desired_playing: Arc<AtomicBool>,
    pub at_eos: Arc<AtomicBool>,
    pub switch_ctx: SwitchContext,
}

#[cfg(target_os = "ios")]
impl IosLayerBusHook {
    pub fn from_engine(
        surface: &VideoSurface,
        shell: Arc<Mutex<PipelineShell>>,
        desired_playing: Arc<AtomicBool>,
        at_eos: Arc<AtomicBool>,
        switch_ctx: SwitchContext,
    ) -> Self {
        Self {
            shell,
            ios_session: surface.ios_session(),
            stored: surface.stored.clone(),
            overlay_sink: surface
                .overlay_sink
                .as_ref()
                .expect("ios overlay sink slot")
                .clone(),
            desired_playing,
            at_eos,
            switch_ctx,
        }
    }

    pub fn refresh_switch_ctx(&mut self, ctx: SwitchContext) {
        self.switch_ctx = ctx;
    }

    pub fn is_overlay_bound(&self) -> bool {
        self.ios_session.is_bound()
    }

    pub fn set_pending_play_after_overlay(&self) {
        self.ios_session.set_pending_play_after_overlay(true);
    }

    pub fn set_buffering_active(&self, active: bool) {
        self.ios_session.set_buffering_active(active);
    }

    /// Marks video ready when sink caps are negotiated; schedules PLAYING if overlay is bound.
    pub fn try_mark_video_ready(&self) {
        let sink = self.overlay_sink.lock().clone();
        if self.ios_session.try_mark_video_ready(&sink) {
            self.schedule_apply();
        }
    }

    fn idle_work(&self) -> IosIdleWork {
        self.ios_session.idle_work(
            self.shell.clone(),
            &IosOverlayPlayIntent {
                desired_playing: self.desired_playing.clone(),
                at_eos: self.at_eos.clone(),
                switch_ctx: self.switch_ctx.clone_for_async(),
            },
        )
    }

    pub fn schedule_apply(&self) {
        self.ios_session.schedule_apply(self.idle_work());
    }

    pub fn schedule_attach(&self) {
        self.ios_session.schedule_attach(self.idle_work());
    }

    /// Routes PLAYING resume through idle [`IosOverlaySession::apply_target_state`].
    pub fn request_playing_resume(&self) {
        self.schedule_apply();
    }

    pub fn try_attach(&self) {
        let play_intent = IosOverlayPlayIntent {
            desired_playing: self.desired_playing.clone(),
            at_eos: self.at_eos.clone(),
            switch_ctx: self.switch_ctx.clone_for_async(),
        };
        match self.ios_session.request_attach(
            self.shell.clone(),
            self.stored.clone(),
            play_intent,
            "READY→PAUSED",
            self.ios_session.overlay_generation().load(Ordering::SeqCst),
        ) {
            Ok(IosLayerAttachOutcome::LayerNotReady) => {
                log::debug!("gst: ios layer attach on READY→PAUSED: layer not ready yet");
            }
            Ok(IosLayerAttachOutcome::Scheduled) => {}
            Ok(IosLayerAttachOutcome::Skipped) => {}
            Err(e) => {
                log::debug!("gst: ios layer attach on READY→PAUSED: {e:#}");
            }
        }
    }
}

/// Play intent forwarded to the Gst thread when a mobile overlay binds (Android/iOS).
#[cfg(any(target_os = "android", target_os = "ios"))]
pub struct MobileOverlayPlayIntent {
    pub desired_playing: Arc<AtomicBool>,
    pub at_eos: Arc<AtomicBool>,
    pub switch_ctx: SwitchContext,
}

/// Cached native overlay handle and platform-specific bind helpers.
pub struct VideoSurface {
    stored: std::sync::Arc<Mutex<Option<usize>>>,
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
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    overlay_sink: Option<std::sync::Arc<Mutex<gst::Element>>>,
}

impl VideoSurface {
    pub fn new(stored: std::sync::Arc<Mutex<Option<usize>>>) -> Self {
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
        }
    }

    #[cfg(target_os = "ios")]
    pub fn ios_session(&self) -> IosOverlaySession {
        self.ios_session.clone()
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn with_overlay_sink_slot(
        stored: std::sync::Arc<Mutex<Option<usize>>>,
        overlay_sink: std::sync::Arc<Mutex<gst::Element>>,
        #[cfg(target_os = "ios")] running: std::sync::Arc<AtomicBool>,
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
        }
    }

    #[cfg(target_os = "macos")]
    pub fn with_macos_overlay_sink(
        stored: std::sync::Arc<Mutex<Option<usize>>>,
        overlay_sink: std::sync::Arc<Mutex<gst::Element>>,
    ) -> Self {
        Self::with_overlay_sink_slot(stored, overlay_sink)
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn set_overlay_sink_slot(&mut self, element: gst::Element) {
        match &self.overlay_sink {
            Some(slot) => *slot.lock() = element,
            None => {
                self.overlay_sink = Some(std::sync::Arc::new(Mutex::new(element)));
            }
        }
    }

    #[cfg(target_os = "macos")]
    pub fn set_macos_overlay_sink(&mut self, element: gst::Element) {
        self.set_overlay_sink_slot(element);
    }

    pub fn stored_handle(&self) -> std::sync::Arc<Mutex<Option<usize>>> {
        self.stored.clone()
    }

    pub fn has_cached_handle(&self) -> bool {
        self.stored.lock().is_some()
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
    #[cfg(target_os = "android")]
    pub fn overlay_ready_for_preroll(&self) -> bool {
        self.has_cached_handle() && self.is_overlay_bound_on_gst()
    }

    /// iOS: host UIView cached — layer attach happens after preroll, not before.
    #[cfg(target_os = "ios")]
    pub fn overlay_ready_for_preroll(&self) -> bool {
        self.has_cached_handle()
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn overlay_ready_for_preroll(&self) -> bool {
        self.has_cached_handle()
    }

    /// Schedules async CALayer attach on the main thread (Tutorial 4 target_state flow).
    #[cfg(target_os = "ios")]
    pub fn schedule_ios_layer_attach(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: MobileOverlayPlayIntent,
    ) -> Result<IosLayerAttachOutcome> {
        let has_layer = {
            let guard = shell.lock();
            guard.video_sink.find_property("layer").is_some()
        };
        if !has_layer {
            return Ok(IosLayerAttachOutcome::Skipped);
        }

        let ios_intent = IosOverlayPlayIntent {
            desired_playing: play_intent.desired_playing.clone(),
            at_eos: play_intent.at_eos.clone(),
            switch_ctx: play_intent.switch_ctx.clone_for_async(),
        };
        let work_generation = self.ios_session.overlay_generation().load(Ordering::SeqCst);
        self.ios_session.request_attach(
            shell,
            self.stored.clone(),
            ios_intent,
            "load/play",
            work_generation,
        )
    }

    /// Attaches `avsamplebufferlayersink` CALayer when preroll has exposed the layer property.
    #[cfg(target_os = "ios")]
    pub fn try_attach_ios_layer_on_gst(
        &self,
        shell: Arc<Mutex<PipelineShell>>,
        play_intent: MobileOverlayPlayIntent,
    ) -> Result<bool> {
        let has_layer = {
            let guard = shell.lock();
            guard.video_sink.find_property("layer").is_some()
        };
        if !has_layer {
            return Ok(self.overlay_bound.load(Ordering::SeqCst));
        }

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
        if let Some(host) = *self.stored.lock() {
            if host != 0 {
                detach_sink_layers_from_host(host);
            }
        }
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
        play_intent: MobileOverlayPlayIntent,
    ) {
        let ios_intent = IosOverlayPlayIntent {
            desired_playing: play_intent.desired_playing,
            at_eos: play_intent.at_eos,
            switch_ctx: play_intent.switch_ctx,
        };
        self.ios_session.drain_pending_play(
            shell,
            &ios_intent,
            self.ios_session.overlay_generation().load(Ordering::SeqCst),
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
        if handle == 0 {
            self.stored.lock().take();
        } else {
            *self.stored.lock() = Some(handle);
        }
    }

    #[cfg(target_os = "macos")]
    pub fn cache_macos_handle(&self, view_ptr: i64) {
        if view_ptr == 0 {
            self.stored.lock().take();
        } else {
            *self.stored.lock() = Some(view_ptr as usize);
        }
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn overlay_sink_slot(&self) -> Option<&std::sync::Arc<Mutex<gst::Element>>> {
        self.overlay_sink.as_ref()
    }

    #[cfg(target_os = "macos")]
    pub fn macos_overlay_sink(&self) -> Option<&std::sync::Arc<Mutex<gst::Element>>> {
        self.overlay_sink_slot()
    }

    /// Applies VideoOverlay on the **main thread** (macOS `osxvideosink`).
    #[cfg(target_os = "macos")]
    pub fn apply_macos_overlay_gstreamer(&self, width: i32, height: i32) -> Result<()> {
        let Some(slot) = self.overlay_sink.as_ref() else {
            return Ok(());
        };
        let sink = slot.lock().clone();
        match *self.stored.lock() {
            None => clear_overlay_window_handle(&sink),
            Some(handle) => {
                set_overlay_window_handle(&sink, handle)?;
                if width > 0 && height > 0 {
                    set_overlay_render_rectangle(&sink, width, height);
                }
                Ok(())
            }
        }
    }

    #[cfg(target_os = "macos")]
    pub fn ensure_overlay_ready(&self) -> Result<()> {
        if self.stored.lock().is_none() {
            log::warn!(
                "macOS overlay handle not cached yet; playback may fail until platform view binds"
            );
        }
        Ok(())
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
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            if let Some(handle) = *self.stored.lock() {
                let (width, height) = self.cached_dimensions();
                #[cfg(target_os = "android")]
                {
                    refresh_mobile_overlay_on_gst(shell, handle, width, height, "rebind")?;
                    self.overlay_bound.store(true, Ordering::SeqCst);
                    crate::diag::logcat_info("gst: overlay rebound on new video_sink");
                }
                #[cfg(target_os = "ios")]
                {
                    let has_layer = shell.video_sink.find_property("layer").is_some();
                    if !has_layer {
                        let _ = crate::platform_view_ios::bind_overlay_on_main_thread(&shell.video_sink, handle, width, height);
                        self.overlay_bound.store(true, Ordering::SeqCst);
                        log::info!("gst: ios glimagesink overlay rebound");
                    }
                }
            }
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "android", target_os = "ios")))]
        {
            if let Some(handle) = *self.stored.lock() {
                apply_overlay_handle(&shell.video_sink, handle, &self.stored)?;
            }
            Ok(())
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
        let old_handle = *self.stored.lock();
        let host_changed = match old_handle {
            Some(h) if h != 0 => h != new_handle,
            _ => false,
        };
        self.set_cached_dimensions(width, height);
        if host_changed {
            if let Some(old) = old_handle {
                detach_sink_layers_from_host(old);
            }
            self.ios_session.reset_for_host_change();
        }
        self.cache_handle(new_handle);
    }

    /// Attaches `avsamplebufferlayersink` CALayer and prerolls on `xhvp-gst` (Swift calls from main).
    #[cfg(target_os = "ios")]
    pub fn apply_ios_overlay_gstreamer(
        &self,
        shell_arc: std::sync::Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
        play_intent: MobileOverlayPlayIntent,
    ) -> Result<()> {
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let (_prev_w, _prev_h) = self.cached_dimensions();
        self.set_cached_dimensions(width, height);
        let host_view = *self.stored.lock();
        let Some(host_view) = host_view else {
            return Ok(());
        };

        let running = play_intent.switch_ctx.running.clone();
        let overlay_bound = self.overlay_bound.clone();

        spawn_on_gst_thread(move || {
            if !running.load(Ordering::SeqCst) {
                return;
            }
            let mut shell = shell_arc.lock();
            let bind_res = crate::platform_view_ios::bind_overlay_on_main_thread(&shell.video_sink, host_view, width, height);
            if let Err(e) = bind_res {
                log::warn!("gst: ios glimagesink overlay bind failed: {e:#}");
            }

            overlay_bound.store(true, Ordering::SeqCst);

            if let Err(e) = maybe_preroll_and_resume_play(&mut shell, &play_intent, host_view, width, height) {
                log::warn!("gst: ios preroll resume failed: {e:#}");
            }
        });
        Ok(())
    }

    #[cfg(target_os = "ios")]
    pub fn notify_ios_overlay(
        &self,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()> {
        self.cache_ios_overlay(handle, width, height);
        Ok(())
    }

    #[cfg(target_os = "android")]
    pub fn notify_android_surface(
        &self,
        shell: std::sync::Arc<Mutex<PipelineShell>>,
        handle: i64,
        width: i32,
        height: i32,
        play_intent: MobileOverlayPlayIntent,
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
            None,
            width,
            height,
            play_intent,
        );
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
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
        shell: std::sync::Arc<Mutex<PipelineShell>>,
        width: i32,
        height: i32,
    ) {
        let stored = self.stored.clone();
        spawn_on_gst_thread(move || {
            let guard = shell.lock();
            if width > 0 && height > 0 {
                set_overlay_render_rectangle(&guard.video_sink, width, height);
            } else if stored.lock().is_some() {
                expose_overlay(&guard.video_sink);
            }
        });
    }

    /// Re-applies VideoOverlay render rectangle + expose after surface resize (Android/iOS).
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub fn schedule_mobile_overlay_rectangle_sync(
        &self,
        shell: std::sync::Arc<Mutex<PipelineShell>>,
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

    /// Shares overlay state for [`super::switch::SwitchContext`].
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
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            overlay_sink: self.overlay_sink.clone(),
        }
    }
}

pub fn apply_overlay_handle(
    video_sink: &gst::Element,
    handle: usize,
    stored: &Mutex<Option<usize>>,
) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        if handle == 0 {
            cache_android_native_window(stored, 0)?;
            clear_overlay_window_handle(video_sink)?;
            return Ok(());
        }
        cache_android_native_window(stored, handle)?;
    }

    #[cfg(not(target_os = "android"))]
    {
        if handle == 0 {
            stored.lock().take();
        } else {
            *stored.lock() = Some(handle);
        }
    }

    if handle == 0 {
        clear_overlay_window_handle(video_sink)?;
    } else {
        set_overlay_window_handle(video_sink, handle)?;
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn assign_overlay_sink(slot: &std::sync::Arc<Mutex<gst::Element>>, element: &gst::Element) {
    *slot.lock() = element.clone();
}

#[cfg(target_os = "android")]
pub fn cache_android_native_window(stored: &Mutex<Option<usize>>, handle: usize) -> Result<()> {
    if handle == 0 {
        if let Some(old) = stored.lock().take() {
            crate::platform_view_android::release_native_window(old);
        }
        return Ok(());
    }
    let mut guard = stored.lock();
    if let Some(old) = *guard {
        if old != handle {
            crate::platform_view_android::release_native_window(old);
        }
    }
    *guard = Some(handle);
    Ok(())
}

/// Rebinds the cached native window on the Gst thread (Android).
#[cfg(target_os = "android")]
pub fn refresh_mobile_overlay_on_gst(
    shell: &PipelineShell,
    handle: usize,
    width: i32,
    height: i32,
    reason: &str,
) -> Result<()> {
    set_overlay_window_handle(&shell.video_sink, handle)?;
    if width > 0 && height > 0 {
        set_overlay_render_rectangle(&shell.video_sink, width, height);
    }
    expose_overlay(&shell.video_sink);
    crate::diag::logcat_info(&format!("gst: overlay refresh {reason} ({width}x{height})"));
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn refresh_mobile_overlay_on_gst(
    _shell: &PipelineShell,
    _handle: usize,
    _width: i32,
    _height: i32,
    _reason: &str,
) -> Result<()> {
    Ok(())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn apply_mobile_overlay_on_gst(
    shell: &mut PipelineShell,
    handle: usize,
    width: i32,
    height: i32,
    play_intent: &MobileOverlayPlayIntent,
    overlay_bound: &AtomicBool,
    last_applied_handle: Option<&AtomicUsize>,
) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        refresh_mobile_overlay_on_gst(shell, handle, width, height, "surface bind")?;
        overlay_bound.store(true, Ordering::SeqCst);
        let (_, current, pending) = shell.pipeline.state(gst::ClockTime::ZERO);
        crate::diag::logcat_info(&format!(
            "gst: overlay applied on Gst thread — pipeline {current:?} pending {pending:?}"
        ));
        maybe_preroll_and_resume_play(shell, play_intent, handle, width, height)
    }
    #[cfg(target_os = "ios")]
    {
        let _ = (
            shell,
            handle,
            width,
            height,
            play_intent,
            overlay_bound,
            last_applied_handle,
        );
        log::warn!("apply_mobile_overlay_on_gst: use apply_ios_overlay_gstreamer on iOS");
        Ok(())
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn maybe_preroll_and_resume_play(
    shell: &mut PipelineShell,
    play_intent: &MobileOverlayPlayIntent,
    handle: usize,
    width: i32,
    height: i32,
) -> Result<()> {
    let (_, current, pending) = shell.pipeline.state(gst::ClockTime::ZERO);
    let desired = play_intent.desired_playing.load(Ordering::SeqCst);

    if pending != gst::State::VoidPending {
        #[cfg(target_os = "android")]
        crate::diag::logcat_info(&format!(
            "gst: overlay bind — pipeline pending {pending:?}, current {current:?}"
        ));
        #[cfg(target_os = "ios")]
        log::info!("gst: overlay bind — pipeline pending {pending:?}, current {current:?}");
        if desired && current == gst::State::Paused {
            #[cfg(target_os = "android")]
            crate::diag::logcat_info("gst: overlay bound — resuming play while pending");
            resume_or_replay_from_eos(shell, &play_intent.at_eos, Some(&play_intent.switch_ctx))?;
            #[cfg(target_os = "android")]
            return refresh_mobile_overlay_on_gst(shell, handle, width, height, "after Playing");
            #[cfg(target_os = "ios")]
            return Ok(());
        }
        return Ok(());
    }

    if current == gst::State::Ready && shell.has_pending_media() {
        #[cfg(target_os = "android")]
        crate::diag::logcat_info("gst: overlay bound — starting Paused preroll");
        #[cfg(target_os = "ios")]
        log::info!("gst: overlay bound — starting Paused preroll");
        set_state_sync(&shell.pipeline, gst::State::Paused)?;
        #[cfg(target_os = "android")]
        refresh_mobile_overlay_on_gst(shell, handle, width, height, "after Paused preroll")?;
    } else if current != gst::State::Ready && current != gst::State::Paused {
        return Ok(());
    }

    if desired {
        #[cfg(target_os = "android")]
        crate::diag::logcat_info("gst: overlay bound — resuming play (desired_playing=true)");
        #[cfg(target_os = "ios")]
        log::info!("gst: overlay bound — resuming play (desired_playing=true)");
        resume_or_replay_from_eos(shell, &play_intent.at_eos, Some(&play_intent.switch_ctx))?;
        #[cfg(target_os = "android")]
        refresh_mobile_overlay_on_gst(shell, handle, width, height, "after Playing")?;
    }
    Ok(())
}

pub fn maybe_preroll_after_overlay_bind(shell: &PipelineShell) -> Result<()> {
    let (_, current, pending) = shell.pipeline.state(gst::ClockTime::ZERO);
    if pending != gst::State::VoidPending {
        return Ok(());
    }
    if current != gst::State::Ready {
        return Ok(());
    }
    if !shell.has_pending_media() {
        return Ok(());
    }
    #[cfg(target_os = "android")]
    crate::diag::logcat_info("gst: overlay bound — starting Paused preroll");
    set_state_sync(&shell.pipeline, gst::State::Paused)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn schedule_mobile_overlay_apply(
    bundle: std::sync::Arc<Mutex<PipelineShell>>,
    stored: std::sync::Arc<Mutex<Option<usize>>>,
    overlay_bound: Arc<AtomicBool>,
    last_applied_handle: Option<Arc<AtomicUsize>>,
    width: i32,
    height: i32,
    play_intent: MobileOverlayPlayIntent,
) {
    spawn_on_gst_thread(move || {
        let mut guard = bundle.lock();
        let Some(handle) = *stored.lock() else {
            overlay_bound.store(false, Ordering::SeqCst);
            return;
        };
        let last_applied_ref = last_applied_handle.as_deref();
        if let Err(e) = apply_mobile_overlay_on_gst(
            &mut guard,
            handle,
            width,
            height,
            &play_intent,
            &overlay_bound,
            last_applied_ref,
        ) {
            overlay_bound.store(false, Ordering::SeqCst);
            #[cfg(target_os = "android")]
            crate::diag::logcat_error(&format!("mobile overlay apply: {e:#}"));
            #[cfg(target_os = "ios")]
            log::warn!("ios overlay apply: {e:#}");
        }
    });
}
