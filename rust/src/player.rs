use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[cfg(not(target_os = "android"))]
use std::thread::JoinHandle;
use std::time::Duration;
#[cfg(not(target_os = "android"))]
use std::time::Instant;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video as gst_video;
use parking_lot::Mutex;

#[cfg(not(target_os = "android"))]
use crate::platform_overlay::{
    attach_overlay_bus_sync_handler, clear_overlay_window_handle, create_platform_video_sink,
    expose_overlay, set_overlay_render_rectangle, set_overlay_window_handle,
};

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
    fn gst_plugin_opengl_register();
}

// GIO TLS backend (glib-networking, OpenSSL) bundled in the iOS framework.
// Without a registered `gio-tls-backend`, `souphttpsrc` cannot complete a TLS
// handshake, so `https://` streams deliver zero bytes and playbin3 fails with
// "Can't typefind stream". This mirrors the Android recipe's
// `G_IO_MODULES := openssl`.
#[cfg(target_os = "ios")]
extern "C" {
    fn _g_io_modules_ensure_extension_points_registered();
    fn g_io_openssl_load(module: *mut std::ffi::c_void);
}

/// Registers the OpenSSL-based GIO TLS backend so `https://` sources work.
#[cfg(target_os = "ios")]
fn register_gio_tls_backend() {
    // SAFETY: both symbols are statically linked from the GStreamer iOS SDK
    // framework. The extension points are registered first so the OpenSSL
    // module can implement `gio-tls-backend`; both calls are idempotent.
    unsafe {
        _g_io_modules_ensure_extension_points_registered();
        g_io_openssl_load(std::ptr::null_mut());
    }
}

#[cfg(target_os = "macos")]
fn register_gio_tls_backend() {
    crate::macos_gio_tls::register_gio_tls_backend(bundled_gstreamer_lib_dir().as_deref());
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
    setup_sandbox_writable_env();
}

/// Writable sandbox paths and registry location shared by iOS/macOS.
#[cfg(any(target_os = "ios", target_os = "macos"))]
fn setup_sandbox_writable_env() {
    use std::path::PathBuf;

    let tmp = std::env::temp_dir();
    let tmp_str = tmp.to_string_lossy().to_string();
    let home: PathBuf = tmp
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| tmp.clone());
    let home_str = home.to_string_lossy().to_string();
    let registry = tmp.join("gstreamer-registry.bin");
    let registry_str = registry.to_string_lossy().to_string();

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

/// Locates GStreamer libraries inside the app bundle, or the system framework.
#[cfg(target_os = "macos")]
fn bundled_gstreamer_lib_dir() -> Option<std::path::PathBuf> {
    use std::path::{Path, PathBuf};

    if let Ok(exe) = std::env::current_exe() {
        if let Some(contents) = exe.parent().and_then(Path::parent) {
            let embedded = contents
                .join("Frameworks")
                .join("GStreamer.framework")
                .join("Versions")
                .join("1.0")
                .join("lib");
            if embedded.is_dir() {
                return Some(embedded);
            }
        }
    }

    let system = PathBuf::from("/Library/Frameworks/GStreamer.framework/Versions/1.0/lib");
    if system.is_dir() {
        return Some(system);
    }
    None
}

/// Prepares macOS sandbox + embedded GStreamer.framework paths before `gst::init()`.
#[cfg(target_os = "macos")]
fn setup_macos_env() {
    setup_sandbox_writable_env();

    if let Some(lib_dir) = bundled_gstreamer_lib_dir() {
        let plugins = lib_dir.join("gstreamer-1.0");
        if plugins.is_dir() {
            std::env::set_var("GST_PLUGIN_SYSTEM_PATH", plugins.to_string_lossy().as_ref());
        }
        let gio_modules = lib_dir.join("gio").join("modules");
        if gio_modules.is_dir() {
            std::env::set_var("GIO_MODULE_DIR", gio_modules.to_string_lossy().as_ref());
        }
    }

    // Prefer VideoToolbox hardware decoders from the bundled applemedia plugin;
    // libav remains as fallback when vtdec is unavailable.
    std::env::set_var(
        "GST_PLUGIN_FEATURE_RANK",
        "vtdec:PRIMARY,vtdemux:PRIMARY,avdec_h264:SECONDARY,avdec_h265:SECONDARY",
    );
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
        gst_plugin_opengl_register();
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
                #[cfg(target_os = "macos")]
                setup_macos_env();
                #[cfg(target_os = "android")]
                {
                    crate::android_gst::ensure_gst_init_android()?;
                }
                #[cfg(not(target_os = "android"))]
                {
                    gst::init().map_err(|e| anyhow!("gst::init failed: {e}"))?;
                }
                // Android plugin registration + JavaVM/Context/ClassLoader setup
                // is performed by the Java-side `GStreamer.init(context)` (see
                // `GStreamerInitProvider`) before this runs; re-registering here
                // would run without the JavaVM and break androidmedia decoding.
                // Register the statically-linked plugins on iOS.
                #[cfg(target_os = "ios")]
                {
                    register_ios_static_plugins();
                    register_gio_tls_backend();
                }
                #[cfg(target_os = "macos")]
                register_gio_tls_backend();
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

/// A single GStreamer `playbin3`-backed video player rendering into a native
/// Platform View via VideoOverlay.
pub struct GstPlayer {
    pipeline: gst::Pipeline,
    video_sink: gst::Element,
    emitter: Arc<Mutex<Option<Emitter>>>,
    rate: Arc<Mutex<f64>>,
    looping: Arc<AtomicBool>,
    desired_playing: Arc<AtomicBool>,
    /// Set when playback reaches EOS; cleared on seek or replay.
    at_eos: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    #[cfg(not(target_os = "android"))]
    bus_thread: Mutex<Option<JoinHandle<()>>>,
    #[cfg(target_os = "android")]
    bus_watch: Mutex<Option<gst::bus::BusWatchGuard>>,
    #[cfg(target_os = "android")]
    position_source: Mutex<Option<gst::glib::SourceId>>,
    native_window: Arc<Mutex<Option<usize>>>,
}

impl GstPlayer {
    /// Creates a player with a platform VideoOverlay sink (window bound later).
    pub fn new() -> Result<Self> {
        crate::diag::logcat_info("GstPlayer::new enter");

        let emitter: Arc<Mutex<Option<Emitter>>> = Arc::new(Mutex::new(None));
        let rate = Arc::new(Mutex::new(1.0));
        let looping = Arc::new(AtomicBool::new(false));
        let desired_playing = Arc::new(AtomicBool::new(false));
        let at_eos = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));

        #[cfg(target_os = "android")]
        let (pipeline, video_sink, bus_watch, position_source) = {
            crate::diag::logcat_info("GstPlayer::new building pipeline on Gst thread");
            let emitter = emitter.clone();
            let looping = looping.clone();
            let desired_playing = desired_playing.clone();
            let at_eos = at_eos.clone();
            let running = running.clone();

            crate::android_gst_runtime::spawn_on_gst_thread_and_wait(move || {
                crate::diag::logcat_info("GstPlayer::new ensure_gst_init (Gst thread)");
                ensure_gst_init()?;
                crate::diag::logcat_info("GstPlayer::new gst ready");
                let (pipeline, video_sink) = build_pipeline(&emitter)?;
                let (bus_watch, position_source) = attach_gst_bus_handlers(
                    &pipeline,
                    &emitter,
                    &looping,
                    &desired_playing,
                    &at_eos,
                    &running,
                )?;
                Ok((pipeline, video_sink, bus_watch, position_source))
            })?
        };

        #[cfg(not(target_os = "android"))]
        let (pipeline, video_sink) = {
            crate::diag::logcat_info("GstPlayer::new ensure_gst_init");
            ensure_gst_init()?;
            crate::diag::logcat_info("GstPlayer::new gst ready");
            build_pipeline(&emitter)?
        };

        log::info!("xue_hua_video_player: GStreamer pipeline built");

        #[cfg(not(target_os = "android"))]
        {
            let overlay_handle = Arc::new(Mutex::new(None));
            attach_overlay_bus_sync_handler(&pipeline, overlay_handle.clone());
            let player = Self {
                pipeline,
                video_sink,
                emitter,
                rate,
                looping,
                desired_playing,
                at_eos,
                running,
                bus_thread: Mutex::new(None),
                native_window: overlay_handle,
            };
            player.spawn_bus_thread();
            log::info!("xue_hua_video_player: player ready");
            return Ok(player);
        }

        #[cfg(target_os = "android")]
        {
            let overlay_handle = Arc::new(Mutex::new(None));
            attach_overlay_bus_sync_handler(&pipeline, overlay_handle.clone());
            let player = Self {
                pipeline,
                video_sink,
                emitter,
                rate,
                looping,
                desired_playing,
                at_eos,
                running,
                bus_watch: Mutex::new(Some(bus_watch)),
                position_source: Mutex::new(Some(position_source)),
                native_window: overlay_handle,
            };
            log::info!("xue_hua_video_player: player ready");
            Ok(player)
        }
    }

    #[cfg(target_os = "android")]
    fn run_on_gst<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&gst::Pipeline) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let pipeline = self.pipeline.clone();
        crate::android_gst_runtime::spawn_on_gst_thread_and_wait(move || f(&pipeline))
    }

    pub fn set_video_overlay_window(&self, window_handle: i64) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.cache_macos_overlay_handle(window_handle);
            return Ok(());
        }
        #[cfg(target_os = "android")]
        {
            let handle = window_handle as usize;
            let video_sink = self.video_sink.clone();
            let stored = self.native_window.clone();
            return self.run_on_gst(move |_| apply_overlay_handle(&video_sink, handle, &stored));
        }
        #[cfg(not(any(target_os = "android", target_os = "macos")))]
        {
            let handle = window_handle as usize;
            apply_overlay_handle(&self.video_sink, handle, &self.native_window)
        }
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
        match *self.native_window.lock() {
            None => clear_overlay_window_handle(&self.video_sink),
            Some(handle) => {
                set_overlay_window_handle(&self.video_sink, handle)?;
                if width > 0 && height > 0 {
                    set_overlay_render_rectangle(&self.video_sink, width, height);
                } else {
                    expose_overlay(&self.video_sink);
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
        #[cfg(target_os = "android")]
        {
            crate::android_gst::ensure_java_gstreamer_for_network(uri)?;
            crate::diag::logcat_info(&format!("set_uri: {uri}"));
            let uri = uri.to_owned();
            let at_eos = self.at_eos.clone();
            return self.run_on_gst(move |pipeline| pipeline_set_uri(pipeline, &uri, &at_eos));
        }
        #[cfg(not(target_os = "android"))]
        self.set_uri_impl(uri)
    }

    fn set_uri_impl(&self, uri: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        self.ensure_macos_overlay_ready()?;
        #[cfg(not(target_os = "macos"))]
        self.rebind_cached_overlay()?;
        pipeline_set_uri(&self.pipeline, uri, &self.at_eos)
    }

    #[cfg(all(not(target_os = "android"), not(target_os = "macos")))]
    fn rebind_cached_overlay(&self) -> Result<()> {
        if let Some(handle) = *self.native_window.lock() {
            apply_overlay_handle(&self.video_sink, handle, &self.native_window)?;
        }
        Ok(())
    }

    pub fn play(&self) -> Result<()> {
        self.desired_playing.store(true, Ordering::SeqCst);
        #[cfg(target_os = "android")]
        {
            let at_eos = self.at_eos.clone();
            let rate = *self.rate.lock();
            return self.run_on_gst(move |pipeline| pipeline_play(pipeline, &at_eos, rate));
        }
        #[cfg(not(target_os = "android"))]
        {
            #[cfg(target_os = "macos")]
            self.ensure_macos_overlay_ready()?;
            #[cfg(not(target_os = "macos"))]
            self.rebind_cached_overlay()?;
            if self.at_eos.swap(false, Ordering::SeqCst) {
                self.seek(0)?;
            }
            self.pipeline.set_state(gst::State::Playing)?;
            Ok(())
        }
    }

    pub fn pause(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        #[cfg(target_os = "android")]
        return self.run_on_gst(|pipeline| {
            pipeline.set_state(gst::State::Paused)?;
            Ok(())
        });
        #[cfg(not(target_os = "android"))]
        {
            self.pipeline.set_state(gst::State::Paused)?;
            Ok(())
        }
    }

    pub fn stop(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.at_eos.store(false, Ordering::SeqCst);
        #[cfg(target_os = "android")]
        {
            let emitter = self.emitter.clone();
            return self.run_on_gst(move |pipeline| {
                pipeline.set_state(gst::State::Ready)?;
                if let Some(cb) = emitter.lock().as_ref() {
                    cb(PlayerEvent::state(PlayerState::Stopped));
                }
                Ok(())
            });
        }
        #[cfg(not(target_os = "android"))]
        {
            self.pipeline.set_state(gst::State::Ready)?;
            self.emit(PlayerEvent::state(PlayerState::Stopped));
            Ok(())
        }
    }

    pub fn seek(&self, position_ms: i64) -> Result<()> {
        #[cfg(target_os = "android")]
        {
            let rate = *self.rate.lock();
            let at_eos = self.at_eos.clone();
            return self.run_on_gst(move |pipeline| {
                pipeline_seek(pipeline, &at_eos, position_ms, rate)
            });
        }
        #[cfg(not(target_os = "android"))]
        {
            self.at_eos.store(false, Ordering::SeqCst);
            let rate = *self.rate.lock();
            pipeline_seek(&self.pipeline, &self.at_eos, position_ms, rate)
        }
    }

    pub fn set_volume(&self, volume: f64) {
        let volume = volume.clamp(0.0, 1.0);
        #[cfg(target_os = "android")]
        {
            let _ = self.run_on_gst(move |pipeline| {
                pipeline.set_property("volume", volume);
                Ok(())
            });
            return;
        }
        #[cfg(not(target_os = "android"))]
        self.pipeline.set_property("volume", volume);
    }

    pub fn set_mute(&self, mute: bool) {
        #[cfg(target_os = "android")]
        {
            let _ = self.run_on_gst(move |pipeline| {
                pipeline.set_property("mute", mute);
                Ok(())
            });
            return;
        }
        #[cfg(not(target_os = "android"))]
        self.pipeline.set_property("mute", mute);
    }

    pub fn set_speed(&self, speed: f64) -> Result<()> {
        let speed = if speed <= 0.0 { 1.0 } else { speed };
        *self.rate.lock() = speed;
        #[cfg(target_os = "android")]
        {
            return self.run_on_gst(move |pipeline| {
                let pos = pipeline
                    .query_position::<gst::ClockTime>()
                    .unwrap_or(gst::ClockTime::ZERO);
                pipeline.seek(
                    speed,
                    gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                    gst::SeekType::Set,
                    pos,
                    gst::SeekType::None,
                    gst::ClockTime::ZERO,
                )?;
                Ok(())
            });
        }
        #[cfg(not(target_os = "android"))]
        {
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
    }

    pub fn set_looping(&self, looping: bool) {
        self.looping.store(looping, Ordering::SeqCst);
    }

    pub fn position_ms(&self) -> i64 {
        #[cfg(target_os = "android")]
        {
            return self
                .run_on_gst(|pipeline| {
                    Ok(pipeline
                        .query_position::<gst::ClockTime>()
                        .map(|p| p.mseconds() as i64)
                        .unwrap_or(0))
                })
                .unwrap_or(0);
        }
        #[cfg(not(target_os = "android"))]
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map(|p| p.mseconds() as i64)
            .unwrap_or(0)
    }

    pub fn duration_ms(&self) -> i64 {
        #[cfg(target_os = "android")]
        {
            return self
                .run_on_gst(|pipeline| {
                    Ok(pipeline
                        .query_duration::<gst::ClockTime>()
                        .map(|d| d.mseconds() as i64)
                        .unwrap_or(0))
                })
                .unwrap_or(0);
        }
        #[cfg(not(target_os = "android"))]
        self.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|d| d.mseconds() as i64)
            .unwrap_or(0)
    }

    #[cfg(not(target_os = "android"))]
    fn spawn_bus_thread(&self) {
        let bus = match self.pipeline.bus() {
            Some(b) => b,
            None => return,
        };
        let pipeline = self.pipeline.clone();
        let emitter = self.emitter.clone();
        let looping = self.looping.clone();
        let desired_playing = self.desired_playing.clone();
        let at_eos = self.at_eos.clone();
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
                                at_eos.store(false, Ordering::SeqCst);
                                let _ = pipeline.seek_simple(
                                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                                    gst::ClockTime::ZERO,
                                );
                            } else {
                                at_eos.store(true, Ordering::SeqCst);
                                emit(PlayerEvent::eos());
                                emit(PlayerEvent::state(PlayerState::Completed));
                            }
                        }
                        MessageView::Error(err) => {
                            log::error!(
                                "GStreamer error: {} ({:?})",
                                err.error(),
                                err.debug()
                            );
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
                            let src = sc.src();
                            if src
                                .as_ref()
                                .map(|s| *s == pipeline.upcast_ref::<gst::Object>())
                                .unwrap_or(false)
                            {
                                emit(PlayerEvent::state(map_state(sc.current())));
                                if sc.current() == gst::State::Paused
                                    || sc.current() == gst::State::Playing
                                {
                                    if let Some(d) = pipeline.query_duration::<gst::ClockTime>()
                                    {
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
        if let Some(bus) = self.pipeline.bus() {
            bus.unset_sync_handler();
        }
        #[cfg(target_os = "android")]
        {
            let pipeline = self.pipeline.clone();
            let _ = crate::android_gst_runtime::spawn_on_gst_thread_and_wait(move || {
                pipeline.set_state(gst::State::Null)?;
                Ok(())
            });
            *self.bus_watch.lock() = None;
            *self.position_source.lock() = None;
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = self.pipeline.set_state(gst::State::Null);
            if let Some(handle) = self.bus_thread.lock().take() {
                let _ = handle.join();
            }
        }
    }
}

fn pipeline_set_uri(
    pipeline: &gst::Pipeline,
    uri: &str,
    at_eos: &AtomicBool,
) -> Result<()> {
    at_eos.store(false, Ordering::SeqCst);
    pipeline.set_state(gst::State::Ready)?;
    pipeline.set_property("uri", uri);
    pipeline.set_state(gst::State::Paused)?;
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
    pipeline.set_state(gst::State::Playing)?;
    Ok(())
}

#[cfg(target_os = "android")]
fn attach_gst_bus_handlers(
    pipeline: &gst::Pipeline,
    emitter: &Arc<Mutex<Option<Emitter>>>,
    looping: &Arc<AtomicBool>,
    desired_playing: &Arc<AtomicBool>,
    at_eos: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
) -> Result<(gst::bus::BusWatchGuard, gst::glib::SourceId)> {
    let bus = pipeline
        .bus()
        .ok_or_else(|| anyhow!("pipeline has no bus"))?;
    let pipeline_bus = pipeline.clone();
    let pipeline_pos = pipeline.clone();
    let emitter_bus = emitter.clone();
    let emitter_pos = emitter.clone();
    let looping = looping.clone();
    let desired_playing = desired_playing.clone();
    let at_eos = at_eos.clone();
    let running_bus = running.clone();
    let running_pos = running.clone();

    let bus_watch = bus
        .add_watch_local(move |_, msg| {
            if !running_bus.load(Ordering::SeqCst) {
                return gst::glib::ControlFlow::Break;
            }
            let emit = |event: PlayerEvent| {
                if let Some(cb) = emitter_bus.lock().as_ref() {
                    cb(event);
                }
            };
            use gst::MessageView;
            match msg.view() {
                MessageView::Eos(..) => {
                    if looping.load(Ordering::SeqCst) {
                        at_eos.store(false, Ordering::SeqCst);
                        let _ = pipeline_bus.seek_simple(
                            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                            gst::ClockTime::ZERO,
                        );
                    } else {
                        at_eos.store(true, Ordering::SeqCst);
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
                        let _ = pipeline_bus.set_state(target);
                    }
                }
                MessageView::DurationChanged(..) => {
                    if let Some(d) = pipeline_bus.query_duration::<gst::ClockTime>() {
                        emit(PlayerEvent::duration(d.mseconds() as i64));
                    }
                }
                MessageView::StateChanged(sc) => {
                    if sc.src().map(|s| s == &pipeline_bus).unwrap_or(false) {
                        emit(PlayerEvent::state(map_state(sc.current())));
                        if sc.current() == gst::State::Paused
                            || sc.current() == gst::State::Playing
                        {
                            if let Some(d) = pipeline_bus.query_duration::<gst::ClockTime>() {
                                emit(PlayerEvent::duration(d.mseconds() as i64));
                            }
                        }
                    }
                }
                _ => {}
            }
            gst::glib::ControlFlow::Continue
        })
        .map_err(|e| anyhow!("bus watch failed: {e}"))?;

    let position_source = gst::glib::timeout_add_local(Duration::from_millis(200), move || {
        if !running_pos.load(Ordering::SeqCst) {
            return gst::glib::ControlFlow::Break;
        }
        if let Some(p) = pipeline_pos.query_position::<gst::ClockTime>() {
            if let Some(cb) = emitter_pos.lock().as_ref() {
                cb(PlayerEvent::position(p.mseconds() as i64));
            }
        }
        gst::glib::ControlFlow::Continue
    });

    Ok((bus_watch, position_source))
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

/// Builds an audio sink bin with `scaletempo` for pitch-preserving rate changes.
/// Falls back to plain `autoaudiosink` when scaletempo is unavailable (e.g.
/// plugins-good / audiofx not installed or not bundled on Android).
fn build_audio_sink_bin() -> Result<gst::Bin> {
    let audio_bin = gst::Bin::new();
    let audiosink = gst::ElementFactory::make("autoaudiosink")
        .build()
        .map_err(|_| anyhow!("failed to create autoaudiosink"))?;

    let head = match (
        gst::ElementFactory::make("scaletempo").build(),
        gst::ElementFactory::make("audioconvert").build(),
        gst::ElementFactory::make("audioresample").build(),
    ) {
        (Ok(scaletempo), Ok(audioconvert), Ok(audioresample)) => {
            audio_bin.add(&scaletempo)?;
            audio_bin.add(&audioconvert)?;
            audio_bin.add(&audioresample)?;
            audio_bin.add(&audiosink)?;
            scaletempo.link(&audioconvert)?;
            audioconvert.link(&audioresample)?;
            audioresample.link(&audiosink)?;
            scaletempo
        }
        _ => {
            gst::warning!(
                gst::CAT_DEFAULT,
                "scaletempo unavailable (install/bundle the audiofx plugin from \
                 gst-plugins-good): playback speed will change pitch."
            );
            audio_bin.add(&audiosink)?;
            audiosink
        }
    };

    let sink_pad = head
        .static_pad("sink")
        .ok_or_else(|| anyhow!("audio sink head has no sink pad"))?;
    let ghost = gst::GhostPad::with_target(&sink_pad)?;
    ghost.set_active(true)?;
    audio_bin.add_pad(&ghost)?;

    Ok(audio_bin)
}

/// Builds a `playbin3` pipeline whose video output uses the platform-recommended sink.
fn build_pipeline(emitter: &Arc<Mutex<Option<Emitter>>>) -> Result<(gst::Pipeline, gst::Element)> {
    crate::diag::logcat_info("build_pipeline: creating playbin3");
    let playbin = gst::ElementFactory::make("playbin3")
        .build()
        .map_err(|_| anyhow!("failed to create playbin3 (is gst-plugins-base installed?)"))?;

    let video_sink = create_platform_video_sink()?;
    attach_video_size_probe(&video_sink, emitter.clone());

    playbin.set_property("video-sink", &video_sink);

    let audio_bin = build_audio_sink_bin()?;
    playbin.set_property("audio-sink", &audio_bin);

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

    let pipeline = playbin
        .dynamic_cast::<gst::Pipeline>()
        .map_err(|_| anyhow!("playbin3 is not a pipeline"))?;
    Ok((pipeline, video_sink))
}

fn attach_video_size_probe(video_sink: &gst::Element, emitter: Arc<Mutex<Option<Emitter>>>) {
    let sink_pad = match video_sink.static_pad("sink") {
        Some(pad) => pad,
        None => return,
    };
    let last_size = Arc::new(Mutex::new((0i32, 0i32)));
    sink_pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
        if let Some(gst::PadProbeData::Event(ref ev)) = info.data {
            if let gst::EventView::Caps(caps) = ev.view() {
                if let Ok(video_info) = gst_video::VideoInfo::from_caps(caps.caps()) {
                    let width = video_info.width() as i32;
                    let height = video_info.height() as i32;
                    let mut ls = last_size.lock();
                    if *ls != (width, height) {
                        *ls = (width, height);
                        if let Some(cb) = emitter.lock().as_ref() {
                            cb(PlayerEvent::video_size(width, height));
                        }
                    }
                }
            }
        }
        gst::PadProbeReturn::Ok
    });
}

fn apply_overlay_handle(
    video_sink: &gst::Element,
    handle: usize,
    stored: &Mutex<Option<usize>>,
) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        if handle == 0 {
            if let Some(old) = stored.lock().take() {
                log::debug!("android overlay: releasing ANativeWindow {old:#x}");
                crate::platform_view_android::release_native_window(old);
            }
            clear_overlay_window_handle(video_sink)?;
            return Ok(());
        }
        let mut guard = stored.lock();
        if let Some(old) = *guard {
            if old != handle {
                log::debug!(
                    "android overlay: surface changed, releasing old ANativeWindow {old:#x}"
                );
                crate::platform_view_android::release_native_window(old);
            }
        }
        log::debug!("android overlay: binding ANativeWindow {handle:#x}");
        *guard = Some(handle);
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
        expose_overlay(video_sink);
    }
    Ok(())
}
