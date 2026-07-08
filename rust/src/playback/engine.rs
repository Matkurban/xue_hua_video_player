//! GStreamer 播放引擎 / GStreamer-backed playback engine.
//!
//! [`PlaybackEngine`] 是 Dart FRB（[`crate::api::player`]）与 xhvp-gst 管线之间的核心：
//! 在专用 Gst 线程上持有 [`crate::playback::gst_context::PlaybackGstContext`]，将
//! `load`/`play`/`pause`/seek/overlay 等命令派发到 [`crate::playback::shell::PipelineShell`]，
//! 经 [`crate::playback::bus`] 向 Dart 发射 [`PlayerEvent`]。
//!
//! [`PlaybackEngine`] is the core between Dart FRB ([`crate::api::player`]) and xhvp-gst:
//! owns [`crate::playback::gst_context::PlaybackGstContext`] on a dedicated Gst thread,
//! dispatches load/play/pause/seek/overlay to [`crate::playback::shell::PipelineShell`],
//! and emits [`PlayerEvent`]s to Dart via [`crate::playback::bus`].

#[cfg(target_os = "android")]
use std::sync::atomic::AtomicI64;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

#[cfg(target_os = "android")]
use crate::gst::ensure_java_gstreamer_for_network;
use crate::gst::{ensure_gst_init, spawn_on_gst_thread_and_wait};
#[cfg(target_os = "android")]
use crate::media::ResolvedSource;
use crate::media::{is_seekable, MediaSource};
use crate::playback::bus::Emitter;
use crate::playback::frame::FrameSink;
use crate::playback::gst::{
    InternalAspectRatioMode, InternalVideoMetadata, InternalVideoOrientationConfig,
};
use crate::playback::gst_context::PlaybackGstContext;
#[cfg(target_os = "ios")]
use crate::playback::overlay::IosLayerBackend;
use crate::playback::play_resume::{overlay_ready_for_play, resume_playing};
use crate::playback::replay::PlayReplayContext;
use crate::playback::shell::{install_uri_shell, teardown_shell, wire_overlay_sync, PipelineShell};
#[cfg(target_os = "android")]
use crate::playback::sink::{android_overlay_size_sync, OverlaySizeSync};
use crate::playback::surface::VideoSurface;
use crate::playback::switch::switch_shell;
use crate::playback::tracks::{read_cached_tracks, TrackCache};
use crate::player_events::{MediaTrack, PlayerEvent, PlayerState, TrackType};

/// 基于 GStreamer 的播放器，经 Platform View 或外部纹理渲染 / GStreamer-backed player rendering via Platform View or external texture.
pub struct PlaybackEngine {
    gst_context: Arc<PlaybackGstContext>,
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
    /// Flutter 外部纹理帧源（Apple/Win/Linux）/ Frame source for Flutter external-texture bridge.
    frame_sink: Arc<FrameSink>,
    /// API 层在 `create_player` 中分配（Android texture JNI）/ Assigned by API layer in `create_player`.
    #[cfg(target_os = "android")]
    player_id: Arc<AtomicI64>,
}

/// [`PlaybackEngine`] 的类型别名（历史命名）/ Type alias for [`PlaybackEngine`] (historical name).
pub type GstPlayer = PlaybackEngine;

impl PlaybackEngine {
    /// 构造播放引擎：初始化 Gst、安装 URI shell、创建 [`PlaybackGstContext`] / Constructs engine: inits Gst, installs URI shell, creates context.
    ///
    /// # 参数 / Parameters
    /// - 无 / None
    ///
    /// # 返回值 / Returns
    /// - 成功：[`PlaybackEngine`] / new engine
    ///
    /// # 错误 / Errors
    /// - GStreamer 初始化或 shell 安装失败 / GStreamer init or shell install failure
    ///
    /// # 线程 / Threading
    /// - 调用线程阻塞等待 Gst 线程完成初始化 / caller blocks on Gst thread init
    ///
    /// # 平台 / Platform
    /// - iOS 注册 layer backend；Android 配置 overlay 尺寸同步 / iOS registers layer backend; Android sets size sync
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
        let frame_sink = FrameSink::new();

        #[cfg(target_os = "android")]
        let player_id = Arc::new(AtomicI64::new(0));
        #[cfg(target_os = "android")]
        let gst_context_slot: Arc<Mutex<Option<Arc<PlaybackGstContext>>>> =
            Arc::new(Mutex::new(None));
        #[cfg(target_os = "android")]
        let overlay_size_sync: Option<OverlaySizeSync> = Some(android_overlay_size_sync(
            player_id.clone(),
            gst_context_slot.clone(),
        ));
        #[cfg(target_os = "android")]
        let overlay_size_sync_for_shell = overlay_size_sync.clone();

        let emitter_init = emitter.clone();
        let looping_init = looping.clone();
        let desired_init = desired_playing.clone();
        let at_eos_init = at_eos.clone();
        let running_init = running.clone();
        let rate_init = rate.clone();
        let overlay_init = native_window.clone();

        let metadata_init = video_metadata.clone();
        let track_cache_init = track_cache.clone();
        let frame_sink_init = frame_sink.clone();

        #[cfg(target_os = "ios")]
        let (shell, surface) = spawn_on_gst_thread_and_wait(move || {
            ensure_gst_init()?;
            let replay = PlayReplayContext {
                desired_playing: desired_init,
                at_eos: at_eos_init,
                running: running_init,
                rate: rate_init,
            };
            let mut surface = VideoSurface::new(overlay_init.clone());
            #[cfg(target_os = "ios")]
            surface.wire_ios_replay_running(replay.running.clone());
            let shell = install_uri_shell(
                &emitter_init,
                &looping_init,
                &replay,
                Some(metadata_init),
                Some(track_cache_init),
                &surface,
                &frame_sink_init,
                #[cfg(target_os = "android")]
                overlay_size_sync.clone(),
            )?;
            let overlay_sink_slot = Arc::new(Mutex::new(shell.clone_video_sink()));
            surface.set_overlay_sink_slot(shell.clone_video_sink());
            wire_overlay_sync(&shell, overlay_init, Some(overlay_sink_slot));
            Ok((shell, surface))
        })?;
        #[cfg(not(target_os = "ios"))]
        let (shell, surface) = spawn_on_gst_thread_and_wait(move || {
            ensure_gst_init()?;
            let replay = PlayReplayContext {
                desired_playing: desired_init,
                at_eos: at_eos_init,
                running: running_init,
                rate: rate_init,
            };
            let surface = VideoSurface::new(overlay_init.clone());
            let shell = install_uri_shell(
                &emitter_init,
                &looping_init,
                &replay,
                Some(metadata_init),
                Some(track_cache_init),
                &surface,
                &frame_sink_init,
                #[cfg(target_os = "android")]
                overlay_size_sync_for_shell.clone(),
            )?;
            wire_overlay_sync(&shell, overlay_init);
            Ok((shell, surface))
        })?;

        log::info!("xue_hua_video_player: PlaybackEngine ready");
        let replay = PlayReplayContext {
            desired_playing: desired_playing.clone(),
            at_eos: at_eos.clone(),
            running: running.clone(),
            rate: rate.clone(),
        };
        let gst_context = Arc::new(PlaybackGstContext::new(
            Arc::new(Mutex::new(shell)),
            surface,
            replay,
            emitter.clone(),
            looping.clone(),
            video_metadata.clone(),
            track_cache.clone(),
            orientation.clone(),
            aspect_mode.clone(),
            frame_sink.clone(),
            #[cfg(target_os = "android")]
            overlay_size_sync,
        ));
        #[cfg(target_os = "android")]
        {
            *gst_context_slot.lock() = Some(gst_context.clone());
        }
        let engine = Self {
            gst_context,
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
            frame_sink,
            #[cfg(target_os = "android")]
            player_id,
        };
        #[cfg(target_os = "ios")]
        engine.register_ios_layer_backend();
        Ok(engine)
    }

    /// 外部纹理帧源；API 层按 `player_id` 注册供原生 C-ABI 访问 / Frame source registered by API under player id for native C-ABI.
    pub fn frame_sink(&self) -> Arc<FrameSink> {
        self.frame_sink.clone()
    }

    #[cfg(target_os = "android")]
    pub fn set_player_id(&self, id: i64) {
        self.player_id.store(id, Ordering::SeqCst);
    }

    #[cfg(target_os = "ios")]
    fn register_ios_layer_backend(&self) {
        self.gst_context
            .surface
            .register_ios_layer_backend(IosLayerBackend::from_context(self.gst_context.clone()));
    }

    /// 在 Gst 线程上执行 shell 操作并阻塞等待结果 / Runs shell operation on Gst thread and blocks for result.
    fn run_on_gst<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut PipelineShell) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let shell = self.gst_context.shell.clone();
        spawn_on_gst_thread_and_wait(move || {
            let mut guard = shell.lock();
            f(&mut guard)
        })
    }

    /// 注册 Dart 事件发射器 / Registers Dart event emitter.
    pub fn set_emitter(&self, emitter: Emitter) {
        *self.emitter.lock() = Some(emitter);
    }

    /// 加载媒体源并可选择自动播放 / Loads media source with optional auto-play.
    ///
    /// # 参数 / Parameters
    /// - `source` — URI 或 Flutter 资产 / URI or Flutter asset
    /// - `auto_play` — 是否加载后立即播放 / whether to play immediately after load
    ///
    /// # 返回值 / Returns
    /// - 成功：`Ok(())` / `Ok(())`
    ///
    /// # 错误 / Errors
    /// - 源解析、overlay 准备或 shell 切换失败 / resolve, overlay, or switch failure
    ///
    /// # 线程 / Threading
    /// - 调用线程阻塞等待 Gst 操作 / caller blocks on Gst dispatch
    ///
    /// # 平台 / Platform
    /// - 纹理平台 autoPlay 立即 preroll；Android 可能延迟至 overlay 绑定 / texture vs overlay deferral
    pub fn load(&self, source: MediaSource, auto_play: bool) -> Result<()> {
        if auto_play {
            self.desired_playing.store(true, Ordering::SeqCst);
        }
        let resolved = source.resolve()?;
        self.seekable
            .store(is_seekable(&resolved), Ordering::SeqCst);
        *self.rate.lock() = 1.0;
        *self.aspect_mode.lock() = InternalAspectRatioMode::Fit;
        self.track_cache.lock().clear();
        #[cfg(target_os = "android")]
        if let ResolvedSource::Uri(ref uri) = resolved {
            ensure_java_gstreamer_for_network(uri)?;
        }
        self.gst_context.surface.ensure_overlay_ready()?;
        #[cfg(target_os = "ios")]
        self.gst_context.surface.mark_media_changed();
        let ctx = self.gst_context.clone_for_async();
        self.run_on_gst(move |pipeline_shell| {
            switch_shell(
                pipeline_shell,
                resolved,
                &ctx.swap,
                &ctx.replay,
                &ctx.surface,
            )?;
            Ok(())
        })?;
        #[cfg(any(
            target_os = "ios",
            target_os = "macos",
            target_os = "windows",
            target_os = "linux"
        ))]
        {
            // Texture rendering: frames flow to the Flutter external texture via
            // the appsink; there is no native surface/overlay to wait for, so
            // preroll + play immediately when requested.
            if auto_play {
                let ctx = self.gst_context.clone_for_async();
                spawn_on_gst_thread_and_wait(move || pipeline_play(&ctx))?;
            }
        }
        #[cfg(not(any(
            target_os = "ios",
            target_os = "macos",
            target_os = "windows",
            target_os = "linux"
        )))]
        {
            let ctx = self.gst_context.clone_for_async();
            if auto_play && self.gst_context.surface.overlay_ready_for_preroll() {
                // resume_playing locks the shell internally — never via run_on_gst.
                spawn_on_gst_thread_and_wait(move || pipeline_play(&ctx))?;
            } else if auto_play {
                log::info!("gst: deferring autoPlay until overlay is bound");
            }
        }
        Ok(())
    }

    #[deprecated(note = "use load(MediaSource::Uri(...)) instead")]
    pub fn set_uri(&self, uri: &str) -> Result<()> {
        self.load(MediaSource::Uri(uri.to_string()), false)
    }

    #[deprecated(note = "use load(MediaSource::FlutterAsset(...)) instead")]
    pub fn set_asset_source(&self, asset_key: &str) -> Result<()> {
        self.load(MediaSource::FlutterAsset(asset_key.to_string()), false)
    }

    /// 开始或恢复播放 / Starts or resumes playback.
    ///
    /// # 参数 / Parameters
    /// - 无 / None
    ///
    /// # 返回值 / Returns
    /// - 成功：`Ok(())`；Android overlay 未绑定时延迟返回 Ok / `Ok(())`; may defer on Android
    ///
    /// # 错误 / Errors
    /// - overlay 准备或 resume 失败 / overlay prep or resume failure
    ///
    /// # 线程 / Threading
    /// - 通过 [`crate::playback::play_resume::resume_playing`] 在 Gst 线程恢复 / resumes on Gst thread
    ///
    /// # 平台 / Platform
    /// - 纹理平台直接 resume；移动端可能先 rebind overlay / direct vs rebind-then-resume
    pub fn play(&self) -> Result<()> {
        self.desired_playing.store(true, Ordering::SeqCst);
        self.gst_context.surface.ensure_overlay_ready()?;

        #[cfg(target_os = "android")]
        if !self.gst_context.surface.has_cached_handle() {
            crate::diag::logcat_info("gst: deferring play until Android overlay is bound");
            return Ok(());
        }

        #[cfg(any(
            target_os = "ios",
            target_os = "macos",
            target_os = "windows",
            target_os = "linux"
        ))]
        {
            // Texture rendering: no native surface to wait for — resume directly.
            let ctx = self.gst_context.clone_for_async();
            spawn_on_gst_thread_and_wait(move || pipeline_play(&ctx))
        }

        #[cfg(not(any(
            target_os = "ios",
            target_os = "macos",
            target_os = "windows",
            target_os = "linux"
        )))]
        {
            let ctx = self.gst_context.clone_for_async();
            // pipeline_play -> resume_playing locks the shell internally, so it must
            // run WITHOUT the shell lock held. Any rebind that needs the locked shell
            // is done first in a scoped lock that is released before pipeline_play.
            spawn_on_gst_thread_and_wait(move || {
                #[cfg(target_os = "android")]
                if !ctx.surface.is_overlay_bound_on_gst() {
                    let guard = ctx.shell.lock();
                    ctx.surface.rebind_cached_overlay(&guard)?;
                }
                #[cfg(not(target_os = "android"))]
                {
                    let guard = ctx.shell.lock();
                    ctx.surface.rebind_cached_overlay(&guard)?;
                }
                pipeline_play(&ctx)
            })
        }
    }

    /// 暂停播放 / Pauses playback.
    pub fn pause(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.run_on_gst(|shell| shell.set_state_sync(gst::State::Paused))
    }

    /// 停止播放（Ready + Stopped 事件）/ Stops playback (Ready + Stopped event).
    pub fn stop(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.at_eos.store(false, Ordering::SeqCst);
        let emitter = self.emitter.clone();
        self.run_on_gst(move |shell| {
            shell.set_state_sync(gst::State::Ready)?;
            if let Some(cb) = emitter.lock().as_ref() {
                cb(PlayerEvent::state(PlayerState::Stopped));
            }
            Ok(())
        })
    }

    /// 跳转到指定位置（毫秒）/ Seeks to position in milliseconds.
    pub fn seek(&self, position_ms: i64) -> Result<()> {
        let rate = *self.rate.lock();
        let at_eos = self.at_eos.clone();
        let desired_playing = self.desired_playing.load(Ordering::SeqCst);
        let emitter = self.emitter.clone();
        self.run_on_gst(move |shell| {
            pipeline_seek(
                shell,
                &at_eos,
                position_ms,
                rate,
                desired_playing,
                Some(&emitter),
            )
        })
    }

    pub fn set_volume(&self, volume: f64) {
        let volume = volume.clamp(0.0, 1.0);
        let _ = self.run_on_gst(move |shell| {
            shell.set_volume(volume);
            Ok(())
        });
    }

    pub fn set_mute(&self, mute: bool) {
        let _ = self.run_on_gst(move |shell| {
            shell.set_mute(mute);
            Ok(())
        });
    }

    pub fn set_speed(&self, speed: f64) -> Result<()> {
        let speed = if speed <= 0.0 { 1.0 } else { speed };
        *self.rate.lock() = speed;
        self.run_on_gst(move |shell| shell.apply_playback_rate(speed))
    }

    pub fn set_looping(&self, looping: bool) {
        self.looping.store(looping, Ordering::SeqCst);
    }

    pub fn position_ms(&self) -> i64 {
        self.run_on_gst(|shell| Ok(shell.query_position_ms()))
            .unwrap_or(0)
    }

    pub fn duration_ms(&self) -> i64 {
        self.run_on_gst(|shell| Ok(shell.query_duration_ms()))
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
                shell.disable_subtitles(&cache);
                return Ok(());
            }
            shell.select_track(&cache, track_type, track_id);
            Ok(())
        })
    }

    pub fn video_metadata(&self) -> crate::player_events::VideoMetadata {
        crate::player_events::VideoMetadata::from(self.video_metadata.lock().clone())
    }

    pub fn set_video_orientation(&self, config: InternalVideoOrientationConfig) -> Result<()> {
        *self.orientation.lock() = config;
        let config = *self.orientation.lock();
        self.run_on_gst(move |shell| {
            if shell.capabilities().orientation {
                shell.apply_orientation(config)?;
            }
            Ok(())
        })
    }

    pub fn set_aspect_ratio_mode(&self, mode: InternalAspectRatioMode) -> Result<()> {
        *self.aspect_mode.lock() = mode;
        self.run_on_gst(move |shell| {
            shell.apply_aspect_ratio(mode);
            Ok(())
        })
    }

    /// 设置原生 overlay 窗口句柄 / Sets native video overlay window handle.
    ///
    /// # 参数 / Parameters
    /// - `window_handle` — 原生视图/窗口指针（整数）/ native view/window pointer as integer
    ///
    /// # 返回值 / Returns
    /// - 成功：`Ok(())` / `Ok(())`
    ///
    /// # 错误 / Errors
    /// - 移动端 surface 通知或桌面 Gst 绑定失败 / mobile notify or desktop bind failure
    ///
    /// # 线程 / Threading
    /// - 桌面平台可能阻塞等待 Gst 线程 / desktop may block on Gst thread
    ///
    /// # 平台 / Platform
    /// - macOS 仅缓存句柄；移动端转发 surface；桌面在 Gst 线程绑定 / per-platform overlay bind
    pub fn set_video_overlay_window(&self, window_handle: i64) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let _ = window_handle;
            return Ok(());
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            return self.notify_mobile_surface(window_handle, 0, 0);
        }
        #[cfg(not(any(target_os = "macos", target_os = "android", target_os = "ios")))]
        {
            let shell = self.gst_context.shell.clone();
            let surface = self.gst_context.surface.clone_for_switch();
            let play_intent = self.gst_context.overlay_intent();
            spawn_on_gst_thread_and_wait(move || {
                surface.set_window_handle_on_gst(shell, window_handle, play_intent)
            })
        }
    }

    /// 同步 VideoOverlay 渲染矩形（原生 surface 尺寸变更后）/ Syncs VideoOverlay render rectangle after native surface resize.
    ///
    /// # 参数 / Parameters
    /// - `width`、`height` — 新尺寸（像素）/ new dimensions in pixels
    ///
    /// # 返回值 / Returns
    /// - 成功：`Ok(())` / `Ok(())`
    ///
    /// # 错误 / Errors
    /// - macOS/iOS overlay 应用失败 / overlay apply failure on Darwin
    ///
    /// # 线程 / Threading
    /// - 调度 Gst 或主线程 overlay 工作（因平台而异）/ schedules platform-specific overlay work
    ///
    /// # 平台 / Platform
    /// - macOS：Texture 路径无 overlay 矩形同步 / macOS texture path: no overlay rectangle sync
    /// - 移动端：缓存尺寸并调度 sync / mobile: cache dimensions and schedule sync
    pub fn sync_video_overlay_rectangle(&self, width: i32, height: i32) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let _ = (width, height);
            return Ok(());
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            self.gst_context
                .surface
                .set_cached_dimensions(width, height);
            let shell = self.gst_context.shell.clone();
            let surface = self.gst_context.surface.clone_for_switch();
            surface.schedule_mobile_overlay_rectangle_sync(shell, width, height);
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "android", target_os = "ios")))]
        {
            let shell = self.gst_context.shell.clone();
            let surface = self.gst_context.surface.clone_for_switch();
            surface.schedule_overlay_rectangle_sync(shell, width, height);
            Ok(())
        }
    }

    pub fn pipeline_capabilities(&self) -> crate::playback::capabilities::PipelineCapabilities {
        self.run_on_gst(|shell| Ok(shell.capabilities()))
            .unwrap_or(crate::playback::capabilities::PipelineCapabilities::PLAYBIN)
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn notify_mobile_surface(&self, handle: i64, width: i32, height: i32) -> Result<()> {
        #[cfg(target_os = "android")]
        {
            let play_intent = self.gst_context.overlay_intent();
            return self.gst_context.surface.notify_android_surface(
                self.gst_context.shell.clone(),
                handle,
                width,
                height,
                play_intent,
            );
        }
        #[cfg(target_os = "ios")]
        return self
            .gst_context
            .surface
            .notify_ios_overlay(handle, width, height);
    }

    #[cfg(target_os = "android")]
    pub fn notify_android_surface(&self, handle: i64, width: i32, height: i32) -> Result<()> {
        self.notify_mobile_surface(handle, width, height)
    }

    #[cfg(target_os = "ios")]
    pub fn notify_ios_overlay(&self, handle: i64, width: i32, height: i32) -> Result<()> {
        self.gst_context
            .surface
            .notify_ios_overlay(handle, width, height)
    }

    #[cfg(target_os = "ios")]
    pub fn apply_ios_overlay_gstreamer(&self, width: i32, height: i32) -> Result<()> {
        let play_intent = self.gst_context.overlay_intent();
        self.gst_context.surface.apply_ios_overlay_gstreamer(
            self.gst_context.shell.clone(),
            width,
            height,
            play_intent,
        )
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        #[cfg(target_os = "ios")]
        self.gst_context.surface.cancel_ios_overlay_work();
        let shell = self.gst_context.shell.clone();
        let _ = spawn_on_gst_thread_and_wait(move || {
            let mut guard = shell.lock();
            if let Some(bus) = guard.pipeline_bus() {
                bus.unset_sync_handler();
            }
            teardown_shell(&mut guard);
            guard.set_state_sync(gst::State::Null)?;
            Ok(())
        });
    }
}

/// 在 shell 上执行 seek 并可选发射位置/缓冲事件 / Performs seek on shell and optionally emits position/buffering events.
fn pipeline_seek(
    shell: &PipelineShell,
    at_eos: &AtomicBool,
    position_ms: i64,
    rate: f64,
    desired_playing: bool,
    emitter: Option<&Mutex<Option<Emitter>>>,
) -> Result<()> {
    at_eos.store(false, Ordering::SeqCst);
    shell.seek_accurate(position_ms, rate)?;
    if let Some(emitter_mutex) = emitter {
        if let Some(cb) = emitter_mutex.lock().as_ref() {
            cb(PlayerEvent::position(position_ms));
            if desired_playing {
                cb(PlayerEvent::state(PlayerState::Buffering));
            }
        }
    }
    Ok(())
}

/// 调用 [`crate::playback::play_resume::resume_playing`] 恢复管线播放 / Invokes resume_playing to start pipeline playback.
fn pipeline_play(ctx: &crate::playback::gst_context::PlaybackGstAsyncSnapshot) -> Result<()> {
    resume_playing(
        ctx.shell.clone(),
        &ctx.replay,
        &ctx.swap,
        &ctx.surface,
        overlay_ready_for_play(&ctx.surface),
    )
}
