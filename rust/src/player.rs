use gstreamer as gst;

use anyhow::{anyhow, Result};
use crate::gst_runtime::spawn_on_gst_thread_and_wait;

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

    std::env::set_var(
        "GST_PLUGIN_FEATURE_RANK",
        "vtdec:PRIMARY,vtdemux:PRIMARY,avdec_h264:SECONDARY,avdec_h265:SECONDARY",
    );
}

/// Registers the statically-linked GStreamer plugins bundled in the iOS
/// `GStreamer.framework`.
#[cfg(target_os = "ios")]
fn register_ios_static_plugins() {
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
                #[cfg(target_os = "ios")]
                setup_ios_env();
                #[cfg(target_os = "macos")]
                setup_macos_env();
                crate::gst_runtime::ensure_gst_runtime();
                #[cfg(target_os = "android")]
                {
                    crate::android_gst::ensure_gst_init_android()?;
                }
                #[cfg(not(target_os = "android"))]
                {
                    spawn_on_gst_thread_and_wait(|| {
                        gst::init().map_err(|e| anyhow!("gst::init failed: {e}"))?;
                        #[cfg(target_os = "ios")]
                        {
                            register_ios_static_plugins();
                            register_gio_tls_backend();
                        }
                        #[cfg(target_os = "macos")]
                        register_gio_tls_backend();
                        Ok(())
                    })?;
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

pub use crate::gst_player::GstPlayer;
