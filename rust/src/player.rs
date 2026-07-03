use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Weak,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gstreamer_video::prelude::VideoFrameExt;
use irondash_run_loop::RunLoop;
use irondash_texture::{BoxedPixelData, SendableTexture, Texture};
use parking_lot::Mutex;

use crate::video_texture::{FrameBuffer, FrameProvider};

/// High-level playback state reported to Dart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Idle,
    Ready,
    Buffering,
    Playing,
    Paused,
    Stopped,
    Completed,
    Error,
}

/// Discriminates which fields of [`PlayerEvent`] are meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerEventKind {
    DurationChanged,
    PositionChanged,
    VideoSize,
    StateChanged,
    Buffering,
    Eos,
    Error,
}

/// A flat event struct pushed to Dart over a broadcast stream.
///
/// Modeled as a struct (rather than a data-carrying enum) so the generated Dart
/// bindings stay dependency-free (no `freezed`). Only the fields relevant to
/// `kind` are populated; others hold defaults.
#[derive(Debug, Clone)]
pub struct PlayerEvent {
    pub kind: PlayerEventKind,
    /// Milliseconds (for `PositionChanged`).
    pub position_ms: i64,
    /// Milliseconds (for `DurationChanged`).
    pub duration_ms: i64,
    /// Pixels (for `VideoSize`).
    pub width: i32,
    /// Pixels (for `VideoSize`).
    pub height: i32,
    /// 0-100 (for `Buffering`).
    pub buffering_percent: i32,
    /// New state (for `StateChanged`).
    pub state: PlayerState,
    /// Human-readable message (for `Error`).
    pub message: String,
}

impl PlayerEvent {
    fn base(kind: PlayerEventKind) -> Self {
        Self {
            kind,
            position_ms: 0,
            duration_ms: 0,
            width: 0,
            height: 0,
            buffering_percent: 0,
            state: PlayerState::Idle,
            message: String::new(),
        }
    }

    fn duration(duration_ms: i64) -> Self {
        Self {
            duration_ms,
            ..Self::base(PlayerEventKind::DurationChanged)
        }
    }

    fn position(position_ms: i64) -> Self {
        Self {
            position_ms,
            ..Self::base(PlayerEventKind::PositionChanged)
        }
    }

    fn video_size(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            ..Self::base(PlayerEventKind::VideoSize)
        }
    }

    fn state(state: PlayerState) -> Self {
        Self {
            state,
            ..Self::base(PlayerEventKind::StateChanged)
        }
    }

    fn buffering(buffering_percent: i32) -> Self {
        Self {
            buffering_percent,
            ..Self::base(PlayerEventKind::Buffering)
        }
    }

    fn eos() -> Self {
        Self::base(PlayerEventKind::Eos)
    }

    fn error(message: String) -> Self {
        Self {
            message,
            ..Self::base(PlayerEventKind::Error)
        }
    }
}

type Emitter = Arc<dyn Fn(PlayerEvent) + Send + Sync>;

// On Android, GStreamer and all of its plugins are compiled statically into a
// single `libgstreamer_android.so`. That library's `JNI_OnLoad` +
// `GStreamer.init(context)` path (registered via `RegisterNatives`) is what
// captures the JavaVM, sets the app `Context`/`ClassLoader`, runs `gst_init`,
// and registers every static plugin - crucially including the `androidmedia`
// (MediaCodec) decoders, which need the JavaVM/ClassLoader to enumerate codecs.
// This is driven from Java at process startup by `GStreamerInitProvider`
// (see `android/src/main/java/.../GStreamerInitProvider.java`), so the Rust
// side must NOT register plugins itself (doing so runs before the Java init and
// without the JavaVM, leaving androidmedia with zero decoders). `gst::init()`
// below is a no-op by the time the Rust core loads, and just satisfies the
// gstreamer-rs bindings.

// On iOS, GStreamer ships as a single *static* `GStreamer.framework`. As on
// Android, statically-linked plugins are not discovered by scanning the
// filesystem, so each plugin must be registered explicitly. Every bundled
// plugin exposes a `gst_plugin_<name>_register` symbol; we register the set
// needed for local + network video playback (playbin3, appsink, common
// containers/parsers, `libav` software decoders, and the HTTP/HLS/RTSP source
// stack). All of these symbols are present in the official GStreamer iOS SDK
// framework.
//
// NOTE: `applemedia` (VideoToolbox hardware decode) is intentionally NOT
// registered: it drags in MoltenVK/Vulkan objects that require a full C++/Metal
// link surface the Rust `cdylib` link (`-nodefaultlibs`) does not provide.
// Software decode via `libav` is used instead; revisit if hardware decode is
// needed (would require linking libc++ and the Vulkan/Metal stack).
#[cfg(target_os = "ios")]
extern "C" {
    fn gst_plugin_coreelements_register();
    fn gst_plugin_app_register();
    fn gst_plugin_typefindfunctions_register();
    fn gst_plugin_playback_register();
    fn gst_plugin_autodetect_register();
    fn gst_plugin_pbtypes_register();
    fn gst_plugin_gio_register();
    fn gst_plugin_videoconvertscale_register();
    fn gst_plugin_videofilter_register();
    fn gst_plugin_videorate_register();
    fn gst_plugin_deinterlace_register();
    fn gst_plugin_videocrop_register();
    fn gst_plugin_audioconvert_register();
    fn gst_plugin_audioresample_register();
    fn gst_plugin_audiorate_register();
    fn gst_plugin_volume_register();
    fn gst_plugin_audiofx_register();
    fn gst_plugin_audioparsers_register();
    fn gst_plugin_videoparsersbad_register();
    fn gst_plugin_isomp4_register();
    fn gst_plugin_matroska_register();
    fn gst_plugin_id3demux_register();
    fn gst_plugin_subparse_register();
    fn gst_plugin_libav_register();
    fn gst_plugin_jpeg_register();
    fn gst_plugin_png_register();
    fn gst_plugin_osxaudio_register();
    fn gst_plugin_soup_register();
    fn gst_plugin_hls_register();
    fn gst_plugin_rtp_register();
    fn gst_plugin_rtpmanager_register();
    fn gst_plugin_rtsp_register();
    fn gst_plugin_udp_register();
    fn gst_plugin_tcp_register();
    fn gst_plugin_srtp_register();
    fn gst_plugin_dtls_register();
}

// GIO TLS backend (glib-networking, OpenSSL) bundled in the iOS framework.
// Without a registered `gio-tls-backend`, `souphttpsrc` cannot complete a TLS
// handshake, so `https://` streams deliver zero bytes and playbin3 fails with
// "Can't typefind stream". This mirrors the Android recipe's
// `G_IO_MODULES := openssl`.
#[cfg(target_os = "ios")]
extern "C" {
    // Registers the standard GIO extension points (incl. `gio-tls-backend`) so
    // the OpenSSL module's `g_io_extension_point_implement` call below succeeds
    // under static linking. GLib-internal but exported by the framework.
    fn _g_io_modules_ensure_extension_points_registered();
    // glib-networking OpenSSL module entry point; registers the TLS/DTLS backend
    // implementations. Takes an (optional) `GIOModule*`; NULL is fine here.
    fn g_io_openssl_load(module: *mut std::ffi::c_void);
}

/// Registers the OpenSSL-based GIO TLS backend so `https://` sources work.
#[cfg(target_os = "ios")]
fn register_ios_tls_backend() {
    // SAFETY: both symbols are statically linked from the GStreamer iOS SDK
    // framework. The extension points are registered first so the OpenSSL
    // module can implement `gio-tls-backend`; both calls are idempotent.
    unsafe {
        _g_io_modules_ensure_extension_points_registered();
        g_io_openssl_load(std::ptr::null_mut());
    }
}

/// Prepares the process environment GStreamer/GLib expect, before `gst::init()`.
///
/// iOS apps launch without the POSIX/XDG environment variables GLib assumes
/// exist. Two concrete failures result:
///
/// * ORC (the SIMD JIT behind `videoconvert`/`audioconvert`/`audioresample`)
///   tries to allocate writable+executable memory, which the iOS Hardened
///   Runtime forbids without the `com.apple.security.cs.allow-jit` entitlement,
///   logging "Failed to create write and exec mmap regions". Setting
///   `ORC_CODE=backup` forces the plain C fallbacks and avoids the JIT path.
/// * With `HOME`/`TMPDIR`/`XDG_*` unset, GLib helpers receive NULL paths and log
///   `g_dir_open ... path != NULL` / `g_filename_to_utf8 ... opsysstring != NULL`
///   CRITICALs. Pointing them all at the app's writable temp sandbox silences
///   these and gives the plugin registry a writable location.
///
/// Must run before `gst::init()` so the values are read during initialization.
#[cfg(target_os = "ios")]
fn setup_ios_env() {
    use std::path::PathBuf;

    // Writable per-app temp sandbox (e.g. .../Application/<UUID>/tmp).
    let tmp = std::env::temp_dir();
    let tmp_str = tmp.to_string_lossy().to_string();
    // The container root is the parent of `tmp`; use it as HOME so caches land
    // under a writable location.
    let home: PathBuf = tmp
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| tmp.clone());
    let home_str = home.to_string_lossy().to_string();
    let registry = tmp.join("gstreamer-registry.bin");
    let registry_str = registry.to_string_lossy().to_string();

    // Called inside `ensure_gst_init`'s `Once` before any GStreamer worker
    // threads are spawned, so there is no concurrent environment access.
    std::env::set_var("ORC_CODE", "backup");

    std::env::set_var("HOME", &home_str);
    std::env::set_var("TMPDIR", &tmp_str);
    std::env::set_var("TMP", &tmp_str);
    std::env::set_var("TEMP", &tmp_str);

    std::env::set_var("XDG_CACHE_HOME", &tmp_str);
    std::env::set_var("XDG_DATA_HOME", &tmp_str);
    std::env::set_var("XDG_CONFIG_HOME", &tmp_str);
    std::env::set_var("XDG_RUNTIME_DIR", &tmp_str);
    std::env::set_var("XDG_DATA_DIRS", &tmp_str);
    std::env::set_var("XDG_CONFIG_DIRS", &tmp_str);

    std::env::set_var("GST_REGISTRY", &registry_str);
}

/// Registers the statically-linked GStreamer plugins bundled in the iOS
/// `GStreamer.framework`.
#[cfg(target_os = "ios")]
fn register_ios_static_plugins() {
    // SAFETY: each symbol is a C plugin registration function statically linked
    // from the GStreamer iOS SDK framework; calling it after `gst::init()` only
    // registers element factories and is idempotent.
    unsafe {
        gst_plugin_coreelements_register();
        gst_plugin_app_register();
        gst_plugin_typefindfunctions_register();
        gst_plugin_playback_register();
        gst_plugin_autodetect_register();
        gst_plugin_pbtypes_register();
        gst_plugin_gio_register();
        gst_plugin_videoconvertscale_register();
        gst_plugin_videofilter_register();
        gst_plugin_videorate_register();
        gst_plugin_deinterlace_register();
        gst_plugin_videocrop_register();
        gst_plugin_audioconvert_register();
        gst_plugin_audioresample_register();
        gst_plugin_audiorate_register();
        gst_plugin_volume_register();
        gst_plugin_audiofx_register();
        gst_plugin_audioparsers_register();
        gst_plugin_videoparsersbad_register();
        gst_plugin_isomp4_register();
        gst_plugin_matroska_register();
        gst_plugin_id3demux_register();
        gst_plugin_subparse_register();
        gst_plugin_libav_register();
        gst_plugin_jpeg_register();
        gst_plugin_png_register();
        gst_plugin_osxaudio_register();
        gst_plugin_soup_register();
        gst_plugin_hls_register();
        gst_plugin_rtp_register();
        gst_plugin_rtpmanager_register();
        gst_plugin_rtsp_register();
        gst_plugin_udp_register();
        gst_plugin_tcp_register();
        gst_plugin_srtp_register();
        gst_plugin_dtls_register();
    }
}

/// Ensures `gst::init()` runs exactly once for the process.
pub fn ensure_gst_init() -> Result<()> {
    use std::sync::Once;
    static INIT: Once = Once::new();
    static mut RESULT: Option<Result<()>> = None;
    // SAFETY: guarded by Once, only written inside call_once.
    unsafe {
        INIT.call_once(|| {
            RESULT = Some((|| {
                // On iOS the environment (ORC_CODE, HOME, TMPDIR, XDG_*) must be
                // prepared before `gst::init()` reads it.
                #[cfg(target_os = "ios")]
                setup_ios_env();
                gst::init().map_err(|e| anyhow!("gst::init failed: {e}"))?;
                // Android plugin registration + JavaVM/Context/ClassLoader setup
                // is performed by the Java-side `GStreamer.init(context)` (see
                // `GStreamerInitProvider`) before this runs; re-registering here
                // would run without the JavaVM and break androidmedia decoding.
                // Register the statically-linked plugins on iOS.
                #[cfg(target_os = "ios")]
                {
                    register_ios_static_plugins();
                    register_ios_tls_backend();
                }
                Ok(())
            })());
        });
        match &*std::ptr::addr_of!(RESULT) {
            Some(Ok(())) => Ok(()),
            Some(Err(e)) => Err(anyhow!("{e}")),
            None => Err(anyhow!("gst init state missing")),
        }
    }
}

/// A single GStreamer `playbin3`-backed video player rendering into a Flutter
/// external texture.
pub struct GstPlayer {
    pipeline: gst::Pipeline,
    texture_id: i64,
    frame_buffer: Arc<FrameBuffer>,
    emitter: Arc<Mutex<Option<Emitter>>>,
    rate: Arc<Mutex<f64>>,
    looping: Arc<AtomicBool>,
    desired_playing: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    bus_thread: Mutex<Option<JoinHandle<()>>>,
    // The sole strong reference to the texture; frames are pushed through it.
    // The appsink callback holds only a `Weak`, so this is guaranteed to be the
    // last owner and can be released on the main thread in `Drop` (see
    // `drop_sendable_on_main`). Wrapped in `Option` so `Drop` can take it.
    sendable: Option<Arc<SendableTexture<BoxedPixelData>>>,
}

/// Releases a `SendableTexture` on the platform (main) thread.
///
/// The texture is wrapped in an irondash `Capsule` that remembers the thread it
/// was created on (the main thread, inside `send_and_wait` in `GstPlayer::new`).
/// Dropping it on any other thread panics with "Capsule was dropped on wrong
/// thread". Since `GstPlayer` is created and dropped on flutter_rust_bridge
/// worker threads, hand the final `Arc` to the main-thread run loop so its
/// `Texture` is destroyed there.
fn drop_sendable_on_main(sendable: Arc<SendableTexture<BoxedPixelData>>) {
    match RunLoop::sender_for_main_thread() {
        Ok(sender) => sender.send(move || drop(sendable)),
        // If the main-thread run loop is unreachable there is no safe thread to
        // drop on; leak rather than panic (this should not happen in practice).
        Err(_) => std::mem::forget(sendable),
    }
}

impl GstPlayer {
    /// Creates a player and its Flutter texture. Must be able to reach the
    /// engine's platform thread (via irondash run loop) to register the texture.
    pub fn new(engine_handle: i64) -> Result<Self> {
        ensure_gst_init()?;

        let frame_buffer = FrameBuffer::new();
        let emitter: Arc<Mutex<Option<Emitter>>> = Arc::new(Mutex::new(None));

        // Create the texture on the platform (main) thread.
        let (texture_id, sendable) = {
            let frame_buffer = frame_buffer.clone();
            let sender = RunLoop::sender_for_main_thread()
                .map_err(|e| anyhow!("cannot reach main thread run loop: {e:?}"))?;
            sender.send_and_wait(
                move || -> Result<(i64, Arc<SendableTexture<BoxedPixelData>>)> {
                    let provider = Arc::new(FrameProvider::new(frame_buffer));
                    let texture = Texture::new_with_provider(engine_handle, provider)
                        .map_err(|e| anyhow!("failed to create texture: {e:?}"))?;
                    let id = texture.id();
                    Ok((id, texture.into_sendable_texture()))
                },
            )?
        };

        // If building the pipeline fails, the last strong reference to the
        // texture would otherwise be dropped here on this worker thread and
        // panic in `Capsule::drop`, masking the real error. Release it on the
        // main thread and propagate the original error instead.
        let pipeline = match build_pipeline(&frame_buffer, &sendable, &emitter) {
            Ok(pipeline) => pipeline,
            Err(e) => {
                drop_sendable_on_main(sendable);
                return Err(e);
            }
        };

        let player = Self {
            pipeline,
            texture_id,
            frame_buffer,
            emitter,
            rate: Arc::new(Mutex::new(1.0)),
            looping: Arc::new(AtomicBool::new(false)),
            desired_playing: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(true)),
            bus_thread: Mutex::new(None),
            sendable: Some(sendable),
        };

        player.spawn_bus_thread();
        Ok(player)
    }

    pub fn texture_id(&self) -> i64 {
        self.texture_id
    }

    pub fn set_emitter(&self, emitter: Emitter) {
        *self.emitter.lock() = Some(emitter);
    }

    fn emit(&self, event: PlayerEvent) {
        if let Some(cb) = self.emitter.lock().as_ref() {
            cb(event);
        }
    }

    /// Loads a media URI. Accepts `file://`, `http(s)://`, `rtsp://`, etc.
    pub fn set_uri(&self, uri: &str) -> Result<()> {
        self.pipeline.set_state(gst::State::Ready)?;
        self.pipeline.set_property("uri", uri);
        // Preroll so duration/size become available and the first frame renders.
        self.pipeline.set_state(gst::State::Paused)?;
        Ok(())
    }

    pub fn play(&self) -> Result<()> {
        self.desired_playing.store(true, Ordering::SeqCst);
        self.pipeline.set_state(gst::State::Playing)?;
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.pipeline.set_state(gst::State::Paused)?;
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.pipeline.set_state(gst::State::Ready)?;
        self.frame_buffer.clear();
        self.emit(PlayerEvent::state(PlayerState::Stopped));
        Ok(())
    }

    pub fn seek(&self, position_ms: i64) -> Result<()> {
        let rate = *self.rate.lock();
        let pos = gst::ClockTime::from_mseconds(position_ms.max(0) as u64);
        self.pipeline.seek(
            rate,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            pos,
            gst::SeekType::None,
            gst::ClockTime::ZERO,
        )?;
        Ok(())
    }

    pub fn set_volume(&self, volume: f64) {
        // playbin exposes a `volume` property in [0.0, 1.0+] and a `mute` flag.
        self.pipeline.set_property("volume", volume.clamp(0.0, 1.0));
    }

    pub fn set_mute(&self, mute: bool) {
        self.pipeline.set_property("mute", mute);
    }

    pub fn set_speed(&self, speed: f64) -> Result<()> {
        let speed = if speed <= 0.0 { 1.0 } else { speed };
        *self.rate.lock() = speed;
        let pos = self
            .pipeline
            .query_position::<gst::ClockTime>()
            .unwrap_or(gst::ClockTime::ZERO);
        self.pipeline.seek(
            speed,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            pos,
            gst::SeekType::None,
            gst::ClockTime::ZERO,
        )?;
        Ok(())
    }

    pub fn set_looping(&self, looping: bool) {
        self.looping.store(looping, Ordering::SeqCst);
    }

    pub fn position_ms(&self) -> i64 {
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map(|p| p.mseconds() as i64)
            .unwrap_or(0)
    }

    pub fn duration_ms(&self) -> i64 {
        self.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|d| d.mseconds() as i64)
            .unwrap_or(0)
    }

    fn spawn_bus_thread(&self) {
        let bus = match self.pipeline.bus() {
            Some(b) => b,
            None => return,
        };
        let pipeline = self.pipeline.clone();
        let emitter = self.emitter.clone();
        let looping = self.looping.clone();
        let desired_playing = self.desired_playing.clone();
        let running = self.running.clone();

        let handle = std::thread::spawn(move || {
            let emit = |event: PlayerEvent| {
                if let Some(cb) = emitter.lock().as_ref() {
                    cb(event);
                }
            };
            let mut last_pos = Instant::now();

            while running.load(Ordering::SeqCst) {
                if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) {
                    use gst::MessageView;
                    match msg.view() {
                        MessageView::Eos(..) => {
                            if looping.load(Ordering::SeqCst) {
                                let _ = pipeline.seek_simple(
                                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                                    gst::ClockTime::ZERO,
                                );
                            } else {
                                emit(PlayerEvent::eos());
                                emit(PlayerEvent::state(PlayerState::Completed));
                            }
                        }
                        MessageView::Error(err) => {
                            emit(PlayerEvent::error(format!(
                                "{} ({:?})",
                                err.error(),
                                err.debug()
                            )));
                            emit(PlayerEvent::state(PlayerState::Error));
                        }
                        MessageView::Buffering(b) => {
                            let percent = b.percent();
                            emit(PlayerEvent::buffering(percent));
                            if desired_playing.load(Ordering::SeqCst) {
                                let target = if percent < 100 {
                                    gst::State::Paused
                                } else {
                                    gst::State::Playing
                                };
                                let _ = pipeline.set_state(target);
                            }
                        }
                        MessageView::DurationChanged(..) => {
                            if let Some(d) = pipeline.query_duration::<gst::ClockTime>() {
                                emit(PlayerEvent::duration(d.mseconds() as i64));
                            }
                        }
                        MessageView::StateChanged(sc) => {
                            if sc.src().map(|s| s == &pipeline).unwrap_or(false) {
                                emit(PlayerEvent::state(map_state(sc.current())));
                                if sc.current() == gst::State::Paused
                                    || sc.current() == gst::State::Playing
                                {
                                    if let Some(d) = pipeline.query_duration::<gst::ClockTime>() {
                                        emit(PlayerEvent::duration(d.mseconds() as i64));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }

                if last_pos.elapsed() >= Duration::from_millis(200) {
                    last_pos = Instant::now();
                    if let Some(p) = pipeline.query_position::<gst::ClockTime>() {
                        emit(PlayerEvent::position(p.mseconds() as i64));
                    }
                }
            }
        });

        *self.bus_thread.lock() = Some(handle);
    }
}

impl Drop for GstPlayer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // Stop dataflow first so the appsink callback (which holds only a `Weak`
        // to the texture) can no longer run, then join the bus thread.
        let _ = self.pipeline.set_state(gst::State::Null);
        if let Some(handle) = self.bus_thread.lock().take() {
            let _ = handle.join();
        }
        // Release the sole strong reference to the texture on the main thread.
        if let Some(sendable) = self.sendable.take() {
            drop_sendable_on_main(sendable);
        }
    }
}

fn map_state(state: gst::State) -> PlayerState {
    match state {
        gst::State::Null => PlayerState::Stopped,
        gst::State::Ready => PlayerState::Ready,
        gst::State::Paused => PlayerState::Paused,
        gst::State::Playing => PlayerState::Playing,
        _ => PlayerState::Idle,
    }
}

/// Configures an HTTP(S) source element: disables TLS certificate verification
/// (`ssl-strict = false`) so `https://` streams with self-signed / invalid
/// certificates play, and sets a conventional `user-agent`. Safe to call on any
/// element; the properties are only touched when present (e.g. `souphttpsrc`).
fn configure_http_source(element: &gst::Element) {
    if element.find_property("ssl-strict").is_some() {
        element.set_property("ssl-strict", false);
    }
    if element.find_property("user-agent").is_some() {
        element.set_property(
            "user-agent",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148",
        );
    }
}

/// Builds a `playbin3` pipeline whose video output is an RGBA `appsink` wrapped
/// in a conversion bin (`videoconvert`, plus a `glcolorconvert ! gldownload` GL
/// front-end on Android so MediaCodec's GL-texture output can be consumed). Each
/// decoded frame is copied into `frame_buffer` and the texture is marked dirty.
fn build_pipeline(
    frame_buffer: &Arc<FrameBuffer>,
    sendable: &Arc<SendableTexture<BoxedPixelData>>,
    emitter: &Arc<Mutex<Option<Emitter>>>,
) -> Result<gst::Pipeline> {
    let playbin = gst::ElementFactory::make("playbin3")
        .build()
        .map_err(|_| anyhow!("failed to create playbin3 (is gst-plugins-base installed?)"))?;

    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .build();
    let appsink = gst_app::AppSink::builder()
        .caps(&caps)
        .max_buffers(1)
        .drop(true)
        .enable_last_sample(false)
        .build();

    let sink_bin = gst::Bin::new();

    // Conversion elements between the bin's sink (ghosted) and the appsink.
    //
    // On Android the video comes from `amcvideodec` (MediaCodec HW decode), which
    // for most Qualcomm/vendor decoders only emits GL textures
    // (`video/x-raw(memory:GLMemory), texture-target=external-oes`) and refuses
    // to negotiate with a plain system-memory `videoconvert`
    // ("Codec only supports GL output but downstream does not" -> not-negotiated,
    // with no software fallback bundled). So the chain must first advertise a
    // GL-capable sink: `glcolorconvert` converts the external-OES texture to RGBA
    // in GL, `gldownload` transfers it to system memory, then `videoconvert`
    // normalizes to the RGBA the appsink requests. glcolorconvert/gldownload pick
    // up the decoder's `GstGLContext` automatically via in-pipeline GstContext
    // propagation. Other platforms (iOS/desktop) decode to system memory, so only
    // `videoconvert` is needed there.
    let mut chain: Vec<gst::Element> = Vec::new();

    #[cfg(target_os = "android")]
    {
        // The GL front-end requires the `opengl` plugin in the bundled
        // libgstreamer_android.so. If it is present, insert
        // `glcolorconvert ! capsfilter(texture-target=2D) ! gldownload` so
        // MediaCodec's GL-texture output can be consumed.
        //
        // The capsfilter is essential: MediaCodec decodes into an
        // `external-oes` GL texture (a SurfaceTexture). Such textures can only
        // be sampled in a shader - they cannot be CPU-mapped or copied
        // (`gstglmemory`: "Cannot map/copy External OES textures"). Without the
        // filter, `glcolorconvert` sees that `gldownload` also advertises
        // GLMemory and simply passes the external-oes texture through unchanged,
        // so `gldownload` then chokes trying to read it. Forcing
        // `texture-target=2D` makes `glcolorconvert` actually sample the
        // external-oes texture into a normal 2D RGBA texture that `gldownload`
        // can transfer to system memory.
        //
        // If the plugin is missing (an older .so not rebuilt with `opengl`),
        // fall back to a plain videoconvert chain: the pipeline still builds
        // (audio/init succeed) instead of hard-failing, and only hardware video
        // decode remains unavailable until the .so is rebuilt.
        use std::str::FromStr;
        let gl_2d_caps =
            gst::Caps::from_str("video/x-raw(memory:GLMemory), texture-target=(string)2D")
                .expect("static GL caps string is valid");
        match (
            gst::ElementFactory::make("glcolorconvert").build(),
            gst::ElementFactory::make("capsfilter")
                .property("caps", &gl_2d_caps)
                .build(),
            gst::ElementFactory::make("gldownload").build(),
        ) {
            (Ok(glcolorconvert), Ok(gl_2d_filter), Ok(gldownload)) => {
                chain.push(glcolorconvert);
                chain.push(gl_2d_filter);
                chain.push(gldownload);
            }
            _ => {
                gst::warning!(
                    gst::CAT_DEFAULT,
                    "opengl plugin missing from libgstreamer_android.so: \
                     MediaCodec GL output cannot be consumed, hardware video \
                     decode will fail to negotiate. Rebuild the .so with the \
                     `opengl` plugin (see android/gstreamer_build/jni/Android.mk)."
                );
            }
        }
    }

    chain.push(
        gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|_| anyhow!("failed to create videoconvert"))?,
    );

    for element in &chain {
        sink_bin.add(element)?;
    }
    sink_bin.add(appsink.upcast_ref::<gst::Element>())?;

    let mut prev: Option<&gst::Element> = None;
    for element in &chain {
        if let Some(p) = prev {
            p.link(element)?;
        }
        prev = Some(element);
    }
    prev.expect("sink chain is never empty (videoconvert is always present)")
        .link(appsink.upcast_ref::<gst::Element>())?;

    let head = chain
        .first()
        .expect("sink chain is never empty (videoconvert is always present)");
    let sink_pad = head
        .static_pad("sink")
        .ok_or_else(|| anyhow!("sink chain head has no sink pad"))?;
    let ghost = gst::GhostPad::with_target(&sink_pad)?;
    ghost.set_active(true)?;
    sink_bin.add_pad(&ghost)?;

    playbin.set_property("video-sink", &sink_bin);

    // Disable TLS certificate verification for HTTP(S) sources on every
    // platform. The bundled GStreamer runtimes ship no CA certificate database
    // (the Android umbrella `.so` and the iOS `GStreamer.framework`), so the
    // TLS backend cannot verify any server certificate and `souphttpsrc` aborts
    // the handshake, delivering zero bytes ("Can't typefind stream"). Setting
    // `ssl-strict = false` makes `souphttpsrc` accept the connection regardless
    // of certificate validity so `https://` streams (incl. self-signed / expired
    // certs) play. Trade-off: no MITM protection.
    //
    // We hook BOTH signals: `source-setup` reliably fires for the primary source
    // element, while `element-setup` also catches sources nested inside
    // `hlsdemux`/adaptivedemux (segment fetches). We also set a conventional
    // `user-agent` because some servers return an empty body for blank UAs.
    playbin.connect("source-setup", false, |values| {
        if let Ok(element) = values[1].get::<gst::Element>() {
            configure_http_source(&element);
        }
        None
    });
    playbin.connect("element-setup", false, |values| {
        if let Ok(element) = values[1].get::<gst::Element>() {
            configure_http_source(&element);
        }
        None
    });

    // Wire the frame callback. The callback holds only a `Weak` reference to the
    // texture so the player's `sendable` field remains the sole strong owner and
    // can be dropped on the main thread (see `drop_sendable_on_main`).
    let fb = frame_buffer.clone();
    let tex: Weak<SendableTexture<BoxedPixelData>> = Arc::downgrade(sendable);
    let emitter = emitter.clone();
    let last_size: Arc<Mutex<(i32, i32)>> = Arc::new(Mutex::new((0, 0)));

    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                let caps = sample.caps().ok_or(gst::FlowError::Error)?;
                let info =
                    gst_video::VideoInfo::from_caps(caps).map_err(|_| gst::FlowError::Error)?;
                let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
                    .map_err(|_| gst::FlowError::Error)?;

                let width = frame.width() as i32;
                let height = frame.height() as i32;
                let src_stride = frame.plane_stride()[0] as usize;
                let plane = frame.plane_data(0).map_err(|_| gst::FlowError::Error)?;
                let row_bytes = width as usize * 4;

                let mut data = vec![0u8; row_bytes * height as usize];
                if src_stride == row_bytes {
                    data.copy_from_slice(&plane[..row_bytes * height as usize]);
                } else {
                    for y in 0..height as usize {
                        let s = y * src_stride;
                        let d = y * row_bytes;
                        data[d..d + row_bytes].copy_from_slice(&plane[s..s + row_bytes]);
                    }
                }

                fb.set(width, height, data);
                if let Some(tex) = tex.upgrade() {
                    tex.mark_frame_available();
                }

                {
                    let mut ls = last_size.lock();
                    if *ls != (width, height) {
                        *ls = (width, height);
                        if let Some(cb) = emitter.lock().as_ref() {
                            cb(PlayerEvent::video_size(width, height));
                        }
                    }
                }

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    let pipeline = playbin
        .dynamic_cast::<gst::Pipeline>()
        .map_err(|_| anyhow!("playbin3 is not a pipeline"))?;
    Ok(pipeline)
}
