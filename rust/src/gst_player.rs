use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::asset_appsrc::build_asset_pipeline;
use crate::asset_resolver::AppSrcFeedState;
use crate::gst_bus::{attach_gst_bus_handlers, Emitter};
use crate::gst_runtime::{spawn_on_gst_thread, spawn_on_gst_thread_and_wait};
use crate::pipeline_builder::build_uri_pipeline;
use crate::pipeline_state::set_state_sync;
use crate::platform_overlay::{
    attach_overlay_bus_sync_handler, clear_overlay_window_handle, expose_overlay,
    set_overlay_render_rectangle, set_overlay_window_handle,
};
use crate::player::ensure_gst_init;
use crate::player_events::{PlayerEvent, PlayerState};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    Uri,
    Asset,
}

struct PipelineBundle {
    pipeline: gst::Pipeline,
    video_sink: gst::Element,
    kind: SourceKind,
    appsrc_feed: Option<Arc<AppSrcFeedState>>,
    bus_watch: Option<gst::bus::BusWatchGuard>,
    position_source: Option<gst::glib::SourceId>,
}

/// GStreamer-backed player rendering into a Platform View via VideoOverlay.
pub struct GstPlayer {
    bundle: Arc<Mutex<PipelineBundle>>,
    /// macOS: video sink clone for main-thread VideoOverlay apply (no Gst-thread hop).
    #[cfg(target_os = "macos")]
    overlay_sink: Arc<Mutex<gst::Element>>,
    emitter: Arc<Mutex<Option<Emitter>>>,
    rate: Arc<Mutex<f64>>,
    looping: Arc<AtomicBool>,
    desired_playing: Arc<AtomicBool>,
    at_eos: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    native_window: Arc<Mutex<Option<usize>>>,
}

impl GstPlayer {
    pub fn new() -> Result<Self> {
        crate::diag::logcat_info("GstPlayer::new enter");

        let emitter: Arc<Mutex<Option<Emitter>>> = Arc::new(Mutex::new(None));
        let rate = Arc::new(Mutex::new(1.0));
        let looping = Arc::new(AtomicBool::new(false));
        let desired_playing = Arc::new(AtomicBool::new(false));
        let at_eos = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let native_window = Arc::new(Mutex::new(None));

        let emitter_init = emitter.clone();
        let looping_init = looping.clone();
        let desired_init = desired_playing.clone();
        let at_eos_init = at_eos.clone();
        let running_init = running.clone();
        let overlay_init = native_window.clone();

        let bundle = spawn_on_gst_thread_and_wait(move || {
            ensure_gst_init()?;
            let bundle = install_uri_pipeline(
                &emitter_init,
                &looping_init,
                &desired_init,
                &at_eos_init,
                &running_init,
            )?;
            attach_overlay_bus_sync_handler(&bundle.pipeline, overlay_init);
            Ok(bundle)
        })?;

        log::info!("xue_hua_video_player: player ready");
        #[cfg(target_os = "macos")]
        let overlay_sink_element = bundle.video_sink.clone();
        Ok(Self {
            bundle: Arc::new(Mutex::new(bundle)),
            #[cfg(target_os = "macos")]
            overlay_sink: Arc::new(Mutex::new(overlay_sink_element)),
            emitter,
            rate,
            looping,
            desired_playing,
            at_eos,
            running,
            native_window,
        })
    }

    fn run_on_gst<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut PipelineBundle) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let bundle = self.bundle.clone();
        spawn_on_gst_thread_and_wait(move || {
            let mut guard = bundle.lock();
            f(&mut guard)
        })
    }

    pub fn set_video_overlay_window(&self, window_handle: i64) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.cache_macos_overlay_handle(window_handle);
            return Ok(());
        }
        #[cfg(target_os = "android")]
        {
            return self.notify_android_surface(window_handle, 0, 0);
        }
        #[cfg(not(any(target_os = "macos", target_os = "android")))]
        {
            let handle = window_handle as usize;
            let stored = self.native_window.clone();
            self.run_on_gst(move |bundle| {
                apply_overlay_handle(&bundle.video_sink, handle, &stored)
            })
        }
    }

    /// Android: caches the native window on the JNI thread and applies VideoOverlay
    /// asynchronously on `xhvp-gst` (must not block the Android main thread).
    #[cfg(target_os = "android")]
    pub fn notify_android_surface(
        &self,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()> {
        if handle == 0 {
            cache_android_native_window(&self.native_window, 0)?;
            let bundle = self.bundle.clone();
            spawn_on_gst_thread(move || {
                let mut guard = bundle.lock();
                if let Err(e) = clear_overlay_window_handle(&guard.video_sink) {
                    log::warn!("android overlay clear: {e:#}");
                }
            });
            return Ok(());
        }
        cache_android_native_window(&self.native_window, handle as usize)?;
        self.schedule_android_overlay_apply(width, height);
        Ok(())
    }

    #[cfg(target_os = "android")]
    fn schedule_android_overlay_apply(&self, width: i32, height: i32) {
        let bundle = self.bundle.clone();
        let stored = self.native_window.clone();
        spawn_on_gst_thread(move || {
            let mut guard = bundle.lock();
            let Some(handle) = *stored.lock() else {
                return;
            };
            if let Err(e) = apply_android_overlay_on_gst(&mut guard, handle, width, height) {
                log::warn!("android overlay apply: {e:#}");
            }
        });
    }

    #[cfg(target_os = "macos")]
    pub fn cache_macos_overlay_handle(&self, view_ptr: i64) {
        if view_ptr == 0 {
            *self.native_window.lock() = None;
        } else {
            *self.native_window.lock() = Some(view_ptr as usize);
        }
    }

    #[cfg(target_os = "macos")]
    pub fn apply_macos_overlay_gstreamer(&self, width: i32, height: i32) -> Result<()> {
        // Called from Swift on the main thread — must not block on xhvp-gst.
        let sink = self.overlay_sink.lock().clone();
        match self.native_window.lock().clone() {
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
    fn ensure_macos_overlay_ready(&self) -> Result<()> {
        if self.native_window.lock().is_none() {
            log::warn!(
                "macOS overlay handle not cached yet; playback may open a standalone window"
            );
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn rebind_cached_overlay(
        _bundle: &PipelineBundle,
        _stored: &Mutex<Option<usize>>,
    ) -> Result<()> {
        // Overlay bind runs on the main thread via Swift apply; bus sync uses cached handle.
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn rebind_cached_overlay(bundle: &PipelineBundle, stored: &Mutex<Option<usize>>) -> Result<()> {
        if let Some(handle) = *stored.lock() {
            apply_overlay_handle(&bundle.video_sink, handle, stored)?;
        }
        Ok(())
    }

    pub fn set_emitter(&self, emitter: Emitter) {
        *self.emitter.lock() = Some(emitter);
    }

    pub fn set_uri(&self, uri: &str) -> Result<()> {
        #[cfg(target_os = "android")]
        crate::android_gst::ensure_java_gstreamer_for_network(uri)?;
        #[cfg(target_os = "macos")]
        self.ensure_macos_overlay_ready()?;

        let uri = uri.to_owned();
        let at_eos = self.at_eos.clone();
        let emitter = self.emitter.clone();
        let looping = self.looping.clone();
        let desired = self.desired_playing.clone();
        let running = self.running.clone();
        let stored = self.native_window.clone();
        #[cfg(target_os = "macos")]
        let overlay_sink = self.overlay_sink.clone();

        self.run_on_gst(move |bundle| {
            if bundle.kind != SourceKind::Uri {
                teardown_pipeline(bundle);
                *bundle = install_uri_pipeline(&emitter, &looping, &desired, &at_eos, &running)?;
                attach_overlay_bus_sync_handler(&bundle.pipeline, stored.clone());
                #[cfg(target_os = "macos")]
                assign_overlay_sink(&overlay_sink, &bundle.video_sink);
            }
            Self::rebind_cached_overlay(bundle, &stored)?;
            let has_overlay = stored.lock().is_some();
            pipeline_set_uri(&bundle.pipeline, &uri, &at_eos, has_overlay)
        })
    }

    pub fn set_asset_source(&self, asset_key: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        self.ensure_macos_overlay_ready()?;

        let asset_key = asset_key.to_owned();
        let at_eos = self.at_eos.clone();
        let emitter = self.emitter.clone();
        let looping = self.looping.clone();
        let desired = self.desired_playing.clone();
        let running = self.running.clone();
        let stored = self.native_window.clone();
        #[cfg(target_os = "macos")]
        let overlay_sink = self.overlay_sink.clone();

        self.run_on_gst(move |bundle| {
            teardown_pipeline(bundle);
            *bundle = install_asset_pipeline(
                &asset_key,
                &emitter,
                &looping,
                &desired,
                &at_eos,
                &running,
            )?;
            attach_overlay_bus_sync_handler(&bundle.pipeline, stored.clone());
            #[cfg(target_os = "macos")]
            assign_overlay_sink(&overlay_sink, &bundle.video_sink);
            Self::rebind_cached_overlay(bundle, &stored)?;
            at_eos.store(false, Ordering::SeqCst);
            #[cfg(target_os = "android")]
            {
                if stored.lock().is_some() {
                    set_state_sync(&bundle.pipeline, gst::State::Paused)?;
                } else {
                    crate::diag::logcat_info(
                        "gst: deferring asset Paused preroll until Android overlay is bound",
                    );
                }
            }
            #[cfg(not(target_os = "android"))]
            set_state_sync(&bundle.pipeline, gst::State::Paused)?;
            Ok(())
        })
    }

    pub fn play(&self) -> Result<()> {
        self.desired_playing.store(true, Ordering::SeqCst);
        #[cfg(target_os = "macos")]
        self.ensure_macos_overlay_ready()?;

        let at_eos = self.at_eos.clone();
        let rate = *self.rate.lock();
        let stored = self.native_window.clone();
        self.run_on_gst(move |bundle| {
            Self::rebind_cached_overlay(bundle, &stored)?;
            pipeline_play(&bundle.pipeline, &at_eos, rate)
        })
    }

    pub fn pause(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.run_on_gst(|bundle| set_state_sync(&bundle.pipeline, gst::State::Paused))
    }

    pub fn stop(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.at_eos.store(false, Ordering::SeqCst);
        let emitter = self.emitter.clone();
        self.run_on_gst(move |bundle| {
            set_state_sync(&bundle.pipeline, gst::State::Ready)?;
            if let Some(cb) = emitter.lock().as_ref() {
                cb(PlayerEvent::state(PlayerState::Stopped));
            }
            Ok(())
        })
    }

    pub fn seek(&self, position_ms: i64) -> Result<()> {
        let rate = *self.rate.lock();
        let at_eos = self.at_eos.clone();
        self.run_on_gst(move |bundle| pipeline_seek(&bundle.pipeline, &at_eos, position_ms, rate))
    }

    pub fn set_volume(&self, volume: f64) {
        let volume = volume.clamp(0.0, 1.0);
        let _ = self.run_on_gst(move |bundle| {
            bundle.pipeline.set_property("volume", volume);
            Ok(())
        });
    }

    pub fn set_mute(&self, mute: bool) {
        let _ = self.run_on_gst(move |bundle| {
            bundle.pipeline.set_property("mute", mute);
            Ok(())
        });
    }

    pub fn set_speed(&self, speed: f64) -> Result<()> {
        let speed = if speed <= 0.0 { 1.0 } else { speed };
        *self.rate.lock() = speed;
        self.run_on_gst(move |bundle| {
            let pos = bundle
                .pipeline
                .query_position::<gst::ClockTime>()
                .unwrap_or(gst::ClockTime::ZERO);
            bundle.pipeline.seek(
                speed,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::SeekType::Set,
                pos,
                gst::SeekType::None,
                gst::ClockTime::ZERO,
            )?;
            Ok(())
        })
    }

    pub fn set_looping(&self, looping: bool) {
        self.looping.store(looping, Ordering::SeqCst);
    }

    pub fn position_ms(&self) -> i64 {
        self.run_on_gst(|bundle| {
            Ok(bundle
                .pipeline
                .query_position::<gst::ClockTime>()
                .map(|p| p.mseconds() as i64)
                .unwrap_or(0))
        })
        .unwrap_or(0)
    }

    pub fn duration_ms(&self) -> i64 {
        self.run_on_gst(|bundle| {
            Ok(bundle
                .pipeline
                .query_duration::<gst::ClockTime>()
                .map(|d| d.mseconds() as i64)
                .unwrap_or(0))
        })
        .unwrap_or(0)
    }
}

impl Drop for GstPlayer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let bundle = self.bundle.clone();
        let _ = spawn_on_gst_thread_and_wait(move || {
            let mut guard = bundle.lock();
            if let Some(bus) = guard.pipeline.bus() {
                bus.unset_sync_handler();
            }
            teardown_pipeline(&mut guard);
            set_state_sync(&guard.pipeline, gst::State::Null)?;
            Ok(())
        });
    }
}

fn install_uri_pipeline(
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    desired_playing: &Arc<AtomicBool>,
    at_eos: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
) -> Result<PipelineBundle> {
    let (pipeline, video_sink) = build_uri_pipeline(emitter)?;
    let (bus_watch, position_source) = attach_gst_bus_handlers(
        &pipeline,
        emitter,
        looping,
        desired_playing,
        at_eos,
        running,
    )?;
    Ok(PipelineBundle {
        pipeline,
        video_sink,
        kind: SourceKind::Uri,
        appsrc_feed: None,
        bus_watch: Some(bus_watch),
        position_source: Some(position_source),
    })
}

fn install_asset_pipeline(
    asset_key: &str,
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    desired_playing: &Arc<AtomicBool>,
    at_eos: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
) -> Result<PipelineBundle> {
    let (pipeline, video_sink, feed) = build_asset_pipeline(asset_key, emitter)?;
    let (bus_watch, position_source) = attach_gst_bus_handlers(
        &pipeline,
        emitter,
        looping,
        desired_playing,
        at_eos,
        running,
    )?;
    Ok(PipelineBundle {
        pipeline,
        video_sink,
        kind: SourceKind::Asset,
        appsrc_feed: Some(feed),
        bus_watch: Some(bus_watch),
        position_source: Some(position_source),
    })
}

fn teardown_pipeline(bundle: &mut PipelineBundle) {
    bundle.bus_watch = None;
    bundle.position_source = None;
    bundle.appsrc_feed = None;
    let _ = bundle.pipeline.set_state(gst::State::Null);
}

fn pipeline_set_uri(
    pipeline: &gst::Pipeline,
    uri: &str,
    at_eos: &AtomicBool,
    has_overlay: bool,
) -> Result<()> {
    at_eos.store(false, Ordering::SeqCst);
    set_state_sync(pipeline, gst::State::Ready)?;
    pipeline.set_property("uri", uri);
    if has_overlay {
        set_state_sync(pipeline, gst::State::Paused)
    } else {
        #[cfg(target_os = "android")]
        crate::diag::logcat_info(
            "gst: deferring URI Paused preroll until Android overlay is bound",
        );
        #[cfg(not(target_os = "android"))]
        set_state_sync(pipeline, gst::State::Paused)?;
        Ok(())
    }
}

fn pipeline_seek(
    pipeline: &gst::Pipeline,
    at_eos: &AtomicBool,
    position_ms: i64,
    rate: f64,
) -> Result<()> {
    at_eos.store(false, Ordering::SeqCst);
    let pos = gst::ClockTime::from_mseconds(position_ms.max(0) as u64);
    pipeline.seek(
        rate,
        gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
        gst::SeekType::Set,
        pos,
        gst::SeekType::None,
        gst::ClockTime::ZERO,
    )?;
    Ok(())
}

fn pipeline_play(pipeline: &gst::Pipeline, at_eos: &AtomicBool, rate: f64) -> Result<()> {
    if at_eos.swap(false, Ordering::SeqCst) {
        pipeline_seek(pipeline, at_eos, 0, rate)?;
    }
    set_state_sync(pipeline, gst::State::Playing)
}

#[cfg(target_os = "macos")]
fn assign_overlay_sink(slot: &Arc<Mutex<gst::Element>>, element: &gst::Element) {
    *slot.lock() = element.clone();
}

fn apply_overlay_handle(
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

#[cfg(target_os = "android")]
fn cache_android_native_window(stored: &Mutex<Option<usize>>, handle: usize) -> Result<()> {
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

#[cfg(target_os = "android")]
fn apply_android_overlay_on_gst(
    bundle: &mut PipelineBundle,
    handle: usize,
    width: i32,
    height: i32,
) -> Result<()> {
    set_overlay_window_handle(&bundle.video_sink, handle)?;
    if width > 0 && height > 0 {
        set_overlay_render_rectangle(&bundle.video_sink, width, height);
    }
    expose_overlay(&bundle.video_sink);
    maybe_preroll_after_overlay_bind(bundle)
}

fn maybe_preroll_after_overlay_bind(bundle: &PipelineBundle) -> Result<()> {
    let (_, current, pending) = bundle.pipeline.state(gst::ClockTime::ZERO);
    if pending != gst::State::VoidPending {
        return Ok(());
    }
    if current != gst::State::Ready {
        return Ok(());
    }
    match bundle.kind {
        SourceKind::Uri => {
            let uri: String = bundle.pipeline.property("uri");
            if uri.is_empty() {
                return Ok(());
            }
        }
        SourceKind::Asset => {}
    }
    #[cfg(target_os = "android")]
    crate::diag::logcat_info("gst: overlay bound — starting Paused preroll");
    set_state_sync(&bundle.pipeline, gst::State::Paused)
}
