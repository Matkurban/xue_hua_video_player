use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::gst_init::ensure_gst_init;
use crate::gst_runtime::{spawn_on_gst_thread, spawn_on_gst_thread_and_wait};
use crate::media::{is_seekable, MediaSource, ResolvedSource};
use crate::playback::bus::Emitter;
use crate::playback::shell::{
    install_asset_shell, install_uri_shell, teardown_shell, wire_overlay_sync, PipelineShell,
    SourceKind,
};
use crate::playback::state::set_state_sync;
use crate::playback::surface::{
    apply_overlay_handle, assign_overlay_sink, maybe_preroll_after_overlay_bind,
};
#[cfg(target_os = "android")]
use crate::playback::surface::{cache_android_native_window, schedule_android_overlay_apply};
use crate::playback::tracks::{collect_playbin_tracks, disable_subtitles, select_playbin_track};
use crate::player_events::{MediaTrack, PlayerEvent, PlayerState, TrackType};
use crate::video::{
    info::InternalVideoMetadata, orientation::InternalAspectRatioMode,
    orientation::InternalVideoOrientationConfig, clear_overlay_window_handle,
    set_overlay_render_rectangle, set_overlay_window_handle,
};
use crate::video::orientation::apply_orientation_to_playbin;

/// GStreamer-backed player rendering into a Platform View via VideoOverlay.
pub struct PlaybackEngine {
    shell: Arc<Mutex<PipelineShell>>,
    #[cfg(target_os = "macos")]
    overlay_sink: Arc<Mutex<gst::Element>>,
    emitter: Arc<Mutex<Option<Emitter>>>,
    rate: Arc<Mutex<f64>>,
    looping: Arc<AtomicBool>,
    desired_playing: Arc<AtomicBool>,
    at_eos: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    native_window: Arc<Mutex<Option<usize>>>,
    seekable: Arc<AtomicBool>,
    video_metadata: Arc<Mutex<InternalVideoMetadata>>,
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
        let orientation = Arc::new(Mutex::new(InternalVideoOrientationConfig::default()));
        let aspect_mode = Arc::new(Mutex::new(InternalAspectRatioMode::default()));

        let emitter_init = emitter.clone();
        let looping_init = looping.clone();
        let desired_init = desired_playing.clone();
        let at_eos_init = at_eos.clone();
        let running_init = running.clone();
        let overlay_init = native_window.clone();

        let metadata_init = video_metadata.clone();
        let shell = spawn_on_gst_thread_and_wait(move || {
            ensure_gst_init()?;
            let shell = install_uri_shell(
                &emitter_init,
                &looping_init,
                &desired_init,
                &at_eos_init,
                &running_init,
                Some(metadata_init),
            )?;
            wire_overlay_sync(&shell, overlay_init);
            Ok(shell)
        })?;

        log::info!("xue_hua_video_player: PlaybackEngine ready");
        #[cfg(target_os = "macos")]
        let overlay_sink_element = shell.video_sink.clone();
        Ok(Self {
            shell: Arc::new(Mutex::new(shell)),
            #[cfg(target_os = "macos")]
            overlay_sink: Arc::new(Mutex::new(overlay_sink_element)),
            emitter,
            rate,
            looping,
            desired_playing,
            at_eos,
            running,
            native_window,
            seekable,
            video_metadata,
            orientation,
            aspect_mode,
        })
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
        match resolved {
            ResolvedSource::Uri(uri) => self.set_uri(&uri),
            ResolvedSource::AppSrc(key) => self.set_asset_source(&key),
        }
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
        let orientation = self.orientation.lock().clone();
        let aspect = *self.aspect_mode.lock();
        let metadata = self.video_metadata.clone();
        #[cfg(target_os = "macos")]
        let overlay_sink = self.overlay_sink.clone();

        self.run_on_gst(move |shell| {
            if shell.kind != SourceKind::Uri {
                teardown_shell(shell);
                *shell = install_uri_shell(
                    &emitter,
                    &looping,
                    &desired,
                    &at_eos,
                    &running,
                    Some(metadata),
                )?;
                wire_overlay_sync(shell, stored.clone());
                #[cfg(target_os = "macos")]
                assign_overlay_sink(&overlay_sink, &shell.video_sink);
            }
            Self::rebind_cached_overlay(shell, &stored)?;
            aspect.apply_to_sink(&shell.video_sink);
            let playbin = shell.pipeline.upcast_ref::<gst::Element>();
            apply_orientation_to_playbin(playbin, orientation)?;
            let has_overlay = stored.lock().is_some();
            pipeline_set_uri(&shell.pipeline, &uri, &at_eos, has_overlay)
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
        let aspect = *self.aspect_mode.lock();
        let metadata = self.video_metadata.clone();
        #[cfg(target_os = "macos")]
        let overlay_sink = self.overlay_sink.clone();

        self.run_on_gst(move |shell| {
            teardown_shell(shell);
            *shell = install_asset_shell(
                &asset_key,
                &emitter,
                &looping,
                &desired,
                &at_eos,
                &running,
                Some(metadata),
            )?;
            wire_overlay_sync(shell, stored.clone());
            #[cfg(target_os = "macos")]
            assign_overlay_sink(&overlay_sink, &shell.video_sink);
            Self::rebind_cached_overlay(shell, &stored)?;
            aspect.apply_to_sink(&shell.video_sink);
            at_eos.store(false, Ordering::SeqCst);
            #[cfg(target_os = "android")]
            {
                if stored.lock().is_some() {
                    set_state_sync(&shell.pipeline, gst::State::Paused)?;
                } else {
                    crate::diag::logcat_info(
                        "gst: deferring asset Paused preroll until Android overlay is bound",
                    );
                }
            }
            #[cfg(not(target_os = "android"))]
            set_state_sync(&shell.pipeline, gst::State::Paused)?;
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
        self.run_on_gst(move |shell| {
            Self::rebind_cached_overlay(shell, &stored)?;
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
        self.run_on_gst(move |shell| {
            let pos = shell
                .pipeline
                .query_position::<gst::ClockTime>()
                .unwrap_or(gst::ClockTime::ZERO);
            shell.pipeline.seek(
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
        self.run_on_gst(|shell| {
            if !shell.is_playbin {
                return Ok(Vec::new());
            }
            let playbin = shell.pipeline.upcast_ref::<gst::Element>();
            Ok(collect_playbin_tracks(playbin))
        })
        .unwrap_or_default()
    }

    pub fn select_track(&self, track_id: u32, track_type: TrackType, enable: bool) -> Result<()> {
        self.run_on_gst(move |shell| {
            if !shell.is_playbin {
                return Ok(());
            }
            let playbin = shell.pipeline.upcast_ref::<gst::Element>();
            if !enable && track_type == TrackType::Subtitle {
                disable_subtitles(playbin);
                return Ok(());
            }
            let track = MediaTrack {
                id: track_id,
                track_type,
                language: String::new(),
                label: String::new(),
                selected: true,
            };
            select_playbin_track(playbin, &track);
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
            if shell.is_playbin {
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
            self.run_on_gst(move |shell| apply_overlay_handle(&shell.video_sink, handle, &stored))
        }
    }

    #[cfg(target_os = "android")]
    pub fn notify_android_surface(
        &self,
        handle: i64,
        width: i32,
        height: i32,
    ) -> Result<()> {
        if handle == 0 {
            cache_android_native_window(&self.native_window, 0)?;
            let shell = self.shell.clone();
            spawn_on_gst_thread(move || {
                let guard = shell.lock();
                if let Err(e) = clear_overlay_window_handle(&guard.video_sink) {
                    log::warn!("android overlay clear: {e:#}");
                }
            });
            return Ok(());
        }
        cache_android_native_window(&self.native_window, handle as usize)?;
        schedule_android_overlay_apply(self.shell.clone(), self.native_window.clone(), width, height);
        Ok(())
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
        _shell: &PipelineShell,
        _stored: &Mutex<Option<usize>>,
    ) -> Result<()> {
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn rebind_cached_overlay(shell: &PipelineShell, stored: &Mutex<Option<usize>>) -> Result<()> {
        if let Some(handle) = *stored.lock() {
            apply_overlay_handle(&shell.video_sink, handle, stored)?;
        }
        Ok(())
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
