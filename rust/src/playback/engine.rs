use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::gst_init::ensure_gst_init;
use crate::gst_runtime::spawn_on_gst_thread_and_wait;
use crate::media::{is_seekable, MediaSource};
use crate::playback::bus::Emitter;
use crate::playback::shell::{install_uri_shell, teardown_shell, wire_overlay_sync, PipelineShell};
use crate::playback::state::set_state_sync;
use crate::playback::surface::VideoSurface;
use crate::playback::switch::{switch_shell, SwitchContext};
use crate::playback::tracks::{
    disable_subtitles_on_pipeline, read_cached_tracks, select_track_on_pipeline, TrackCache,
};
use crate::player_events::{MediaTrack, PlayerEvent, PlayerState, TrackType};
use crate::video::{
    info::InternalVideoMetadata, orientation::InternalAspectRatioMode,
    orientation::InternalVideoOrientationConfig,
};
use crate::video::orientation::apply_orientation_to_playbin;

/// GStreamer-backed player rendering into a Platform View via VideoOverlay.
pub struct PlaybackEngine {
    shell: Arc<Mutex<PipelineShell>>,
    surface: VideoSurface,
    emitter: Arc<Mutex<Option<Emitter>>>,
    rate: Arc<Mutex<f64>>,
    looping: Arc<AtomicBool>,
    desired_playing: Arc<AtomicBool>,
    at_eos: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    seekable: Arc<AtomicBool>,
    video_metadata: Arc<Mutex<InternalVideoMetadata>>,
    track_cache: Arc<Mutex<TrackCache>>,
    orientation: Arc<Mutex<InternalVideoOrientationConfig>>,
    aspect_mode: Arc<Mutex<InternalAspectRatioMode>>,
}

pub type GstPlayer = PlaybackEngine;

impl PlaybackEngine {
    pub fn new() -> Result<Self> {
        crate::diag::logcat_info("PlaybackEngine::new enter");

        let emitter: Arc<Mutex<Option<Emitter>>> = Arc::new(Mutex::new(None));
        let rate = Arc::new(Mutex::new(1.0));
        let looping = Arc::new(AtomicBool::new(false));
        let desired_playing = Arc::new(AtomicBool::new(false));
        let at_eos = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let native_window = Arc::new(Mutex::new(None));
        let seekable = Arc::new(AtomicBool::new(true));
        let video_metadata = Arc::new(Mutex::new(InternalVideoMetadata::default()));
        let track_cache = Arc::new(Mutex::new(TrackCache::default()));
        let orientation = Arc::new(Mutex::new(InternalVideoOrientationConfig::default()));
        let aspect_mode = Arc::new(Mutex::new(InternalAspectRatioMode::default()));

        let emitter_init = emitter.clone();
        let looping_init = looping.clone();
        let desired_init = desired_playing.clone();
        let at_eos_init = at_eos.clone();
        let running_init = running.clone();
        let overlay_init = native_window.clone();

        let metadata_init = video_metadata.clone();
        let track_cache_init = track_cache.clone();
        let shell = spawn_on_gst_thread_and_wait(move || {
            ensure_gst_init()?;
            let shell = install_uri_shell(
                &emitter_init,
                &looping_init,
                &desired_init,
                &at_eos_init,
                &running_init,
                Some(metadata_init),
                Some(track_cache_init),
            )?;
            wire_overlay_sync(&shell, overlay_init);
            Ok(shell)
        })?;

        log::info!("xue_hua_video_player: PlaybackEngine ready");
        #[cfg(target_os = "macos")]
        let surface = VideoSurface::with_macos_overlay_sink(
            native_window,
            Arc::new(Mutex::new(shell.video_sink.clone())),
        );
        #[cfg(not(target_os = "macos"))]
        let surface = VideoSurface::new(native_window);
        Ok(Self {
            shell: Arc::new(Mutex::new(shell)),
            surface,
            emitter,
            rate,
            looping,
            desired_playing,
            at_eos,
            running,
            seekable,
            video_metadata,
            track_cache,
            orientation,
            aspect_mode,
        })
    }

    fn switch_context(&self) -> SwitchContext {
        SwitchContext {
            emitter: self.emitter.clone(),
            looping: self.looping.clone(),
            desired_playing: self.desired_playing.clone(),
            at_eos: self.at_eos.clone(),
            running: self.running.clone(),
            metadata: self.video_metadata.clone(),
            track_cache: self.track_cache.clone(),
            orientation: *self.orientation.lock(),
            aspect: *self.aspect_mode.lock(),
            surface: self.surface.clone_for_switch(),
        }
    }

    fn run_on_gst<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut PipelineShell) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let shell = self.shell.clone();
        spawn_on_gst_thread_and_wait(move || {
            let mut guard = shell.lock();
            f(&mut guard)
        })
    }

    pub fn set_emitter(&self, emitter: Emitter) {
        *self.emitter.lock() = Some(emitter);
    }

    pub fn load(&self, source: MediaSource) -> Result<()> {
        let resolved = source.resolve()?;
        self.seekable
            .store(is_seekable(&resolved), Ordering::SeqCst);
        *self.rate.lock() = 1.0;
        self.track_cache.lock().clear();
        #[cfg(target_os = "android")]
        if let ResolvedSource::Uri(ref uri) = resolved {
            crate::android_gst::ensure_java_gstreamer_for_network(uri)?;
        }
        self.surface.ensure_overlay_ready()?;
        let ctx = self.switch_context();
        self.run_on_gst(move |shell| switch_shell(shell, resolved, &ctx))
    }

    #[deprecated(note = "use load(MediaSource::Uri(...)) instead")]
    pub fn set_uri(&self, uri: &str) -> Result<()> {
        self.load(MediaSource::Uri(uri.to_string()))
    }

    #[deprecated(note = "use load(MediaSource::FlutterAsset(...)) instead")]
    pub fn set_asset_source(&self, asset_key: &str) -> Result<()> {
        self.load(MediaSource::FlutterAsset(asset_key.to_string()))
    }

    pub fn play(&self) -> Result<()> {
        self.desired_playing.store(true, Ordering::SeqCst);
        self.surface.ensure_overlay_ready()?;

        let at_eos = self.at_eos.clone();
        let rate = *self.rate.lock();
        let surface = self.surface.clone_for_switch();
        self.run_on_gst(move |shell| {
            surface.rebind_cached_overlay(shell)?;
            pipeline_play(&shell.pipeline, &at_eos, rate)
        })
    }

    pub fn pause(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.run_on_gst(|shell| set_state_sync(&shell.pipeline, gst::State::Paused))
    }

    pub fn stop(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.at_eos.store(false, Ordering::SeqCst);
        let emitter = self.emitter.clone();
        self.run_on_gst(move |shell| {
            set_state_sync(&shell.pipeline, gst::State::Ready)?;
            if let Some(cb) = emitter.lock().as_ref() {
                cb(PlayerEvent::state(PlayerState::Stopped));
            }
            Ok(())
        })
    }

    pub fn seek(&self, position_ms: i64) -> Result<()> {
        let rate = *self.rate.lock();
        let at_eos = self.at_eos.clone();
        self.run_on_gst(move |shell| pipeline_seek(&shell.pipeline, &at_eos, position_ms, rate))
    }

    pub fn set_volume(&self, volume: f64) {
        let volume = volume.clamp(0.0, 1.0);
        let _ = self.run_on_gst(move |shell| {
            shell.pipeline.set_property("volume", volume);
            Ok(())
        });
    }

    pub fn set_mute(&self, mute: bool) {
        let _ = self.run_on_gst(move |shell| {
            shell.pipeline.set_property("mute", mute);
            Ok(())
        });
    }

    pub fn set_speed(&self, speed: f64) -> Result<()> {
        let speed = if speed <= 0.0 { 1.0 } else { speed };
        *self.rate.lock() = speed;
        self.run_on_gst(move |shell| apply_playback_rate(&shell.pipeline, speed))
    }

    pub fn set_looping(&self, looping: bool) {
        self.looping.store(looping, Ordering::SeqCst);
    }

    pub fn position_ms(&self) -> i64 {
        self.run_on_gst(|shell| {
            Ok(shell
                .pipeline
                .query_position::<gst::ClockTime>()
                .map(|p| p.mseconds() as i64)
                .unwrap_or(0))
        })
        .unwrap_or(0)
    }

    pub fn duration_ms(&self) -> i64 {
        self.run_on_gst(|shell| {
            Ok(shell
                .pipeline
                .query_duration::<gst::ClockTime>()
                .map(|d| d.mseconds() as i64)
                .unwrap_or(0))
        })
        .unwrap_or(0)
    }

    pub fn is_seekable(&self) -> bool {
        self.seekable.load(Ordering::SeqCst)
    }

    pub fn tracks(&self) -> Vec<MediaTrack> {
        let track_cache = self.track_cache.clone();
        self.run_on_gst(move |shell| {
            if !shell.capabilities().tracks {
                return Ok(Vec::new());
            }
            Ok(read_cached_tracks(&track_cache))
        })
        .unwrap_or_default()
    }

    pub fn select_track(&self, track_id: u32, track_type: TrackType, enable: bool) -> Result<()> {
        let track_cache = self.track_cache.clone();
        self.run_on_gst(move |shell| {
            if !shell.capabilities().tracks {
                return Ok(());
            }
            let cache = track_cache.lock().clone();
            if !enable && track_type == TrackType::Subtitle {
                disable_subtitles_on_pipeline(&shell.pipeline, &cache);
                return Ok(());
            }
            select_track_on_pipeline(&shell.pipeline, &cache, track_type, track_id);
            Ok(())
        })
    }

    pub fn video_metadata(&self) -> crate::player_events::VideoMetadata {
        crate::player_events::VideoMetadata::from(self.video_metadata.lock().clone())
    }

    pub fn set_video_orientation(
        &self,
        config: InternalVideoOrientationConfig,
    ) -> Result<()> {
        *self.orientation.lock() = config;
        let config = *self.orientation.lock();
        self.run_on_gst(move |shell| {
            if shell.capabilities().orientation {
                apply_orientation_to_playbin(shell.pipeline.upcast_ref::<gst::Element>(), config)?;
            }
            Ok(())
        })
    }

    pub fn set_aspect_ratio_mode(&self, mode: InternalAspectRatioMode) -> Result<()> {
        *self.aspect_mode.lock() = mode;
        self.run_on_gst(move |shell| {
            mode.apply_to_sink(&shell.video_sink);
            Ok(())
        })
    }

    pub fn set_video_overlay_window(&self, window_handle: i64) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.surface.cache_macos_handle(window_handle);
            return Ok(());
        }
        #[cfg(target_os = "android")]
        {
            return self
                .surface
                .notify_android_surface(self.shell.clone(), window_handle, 0, 0);
        }
        #[cfg(not(any(target_os = "macos", target_os = "android")))]
        {
            let surface = self.surface.clone_for_switch();
            self.run_on_gst(move |shell| surface.set_window_handle_on_gst(shell, window_handle))
        }
    }

    #[cfg(target_os = "android")]
    pub fn notify_android_surface(
        &self,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()> {
        self.surface
            .notify_android_surface(self.shell.clone(), handle, width, height)
    }

    #[cfg(target_os = "macos")]
    pub fn cache_macos_overlay_handle(&self, view_ptr: i64) {
        self.surface.cache_macos_handle(view_ptr);
    }

    #[cfg(target_os = "macos")]
    pub fn apply_macos_overlay_gstreamer(&self, width: i32, height: i32) -> Result<()> {
        self.surface.apply_macos_overlay_gstreamer(width, height)
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let shell = self.shell.clone();
        let _ = spawn_on_gst_thread_and_wait(move || {
            let mut guard = shell.lock();
            if let Some(bus) = guard.pipeline.bus() {
                bus.unset_sync_handler();
            }
            teardown_shell(&mut guard);
            set_state_sync(&guard.pipeline, gst::State::Null)?;
            Ok(())
        });
    }
}

fn apply_playback_rate(pipeline: &gst::Pipeline, rate: f64) -> Result<()> {
    pipeline.seek(
        rate,
        gst::SeekFlags::INSTANT_RATE_CHANGE,
        gst::SeekType::None,
        gst::ClockTime::ZERO,
        gst::SeekType::None,
        gst::ClockTime::ZERO,
    )?;
    Ok(())
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
