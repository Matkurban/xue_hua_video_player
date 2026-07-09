//! Android GStreamer 初始化（Java [`GStreamerInitProvider`] 已在进程启动时调用 `GStreamer.init`）。
//!
//! Android GStreamer init when [`GStreamerInitProvider`] has already called
//! `GStreamer.init(context)` at process startup.
//!
//! 将 gstreamer-rs 与 C 运行时（Java 提供方或 `gst::init`）同步，并在首次网络
//! 播放前确认 Java 上下文已就绪。

use anyhow::{anyhow, Result};
use gstreamer as gst;

static JAVA_GST_CONTEXT_ENSURED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static REQWEST_RUNTIME_WARMED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static GL_DISPLAY_WARMED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Lightweight URL probed at process start to force `reqwesthttpsrc`'s Tokio `RUNTIME`
/// LazyLock before SDK-heavy code exhausts Bionic pthread keys.
#[cfg(target_os = "android")]
const REQWEST_WARMUP_URL: &str = "https://www.gstatic.com/generate_204";

/// Android bundles `reqwesthttpsrc` (not `souphttpsrc`). Default rank is marginal;
/// promote to primary so `playbin` selects it for `http(s)://` URIs.
fn ensure_android_http_uri_handler() {
    use gstreamer::prelude::*;

    match gst::ElementFactory::find("reqwesthttpsrc") {
        Some(factory) => {
            factory.set_rank(gst::Rank::PRIMARY);
            crate::diag::logcat_info(
                "gst: Android HTTP URI handler — reqwesthttpsrc only (soup not bundled)",
            );
        }
        None => {
            crate::diag::logcat_error(
                "gst: reqwesthttpsrc missing — Android HTTP(S) playback unavailable",
            );
        }
    }
}

/// 将 gstreamer-rs 与 C 运行时同步（Java 提供方或 `gst::init`）。
/// Syncs gstreamer-rs with the C runtime (Java provider or `gst::init`).
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// - 成功时返回 `Ok(())` / `Ok(())` on success.
///
/// # 错误 / Errors
/// - `gst::init()` 失败时返回错误 / Returns an error if `gst::init()` fails.
///
/// # 线程 / Threading
/// - 会先启动 `xhvp-gst` 运行时线程 / Starts the `xhvp-gst` runtime thread first.
/// - `gst::init()` 在调用线程执行 / `gst::init()` runs on the calling thread.
///
/// # 平台 / Platform
/// - 仅 **Android** / **Android** only.
pub fn ensure_gst_init_android() -> Result<()> {
    crate::diag::logcat_info("gst: ensure_gst_init_android enter");

    #[cfg(target_os = "android")]
    super::runtime::ensure_gst_runtime();

    let c_initialized = unsafe { gst::ffi::gst_is_initialized() != gst::glib::ffi::GFALSE };
    crate::diag::logcat_info(&format!(
        "gst: gst_is_initialized()={c_initialized} INITIALIZED={}",
        gst::INITIALIZED.load(std::sync::atomic::Ordering::SeqCst)
    ));

    match gst::init() {
        Ok(()) => {
            crate::diag::logcat_info("gst: gst::init() / gst_init_check ok");
            ensure_android_http_uri_handler();
            Ok(())
        }
        Err(e) => {
            crate::diag::logcat_error(&format!("gst: gst::init() failed: {e}"));
            Err(anyhow!("gst::init failed: {e}"))
        }
    }
}

/// 在首次 HTTP(S) `playbin` URI 之前，确认 Java `GStreamer.init(Context)` 已执行。
/// Before the first HTTP(S) `playbin` URI, ensure Java `GStreamer.init(Context)` ran.
///
/// # 参数 / Parameters
/// - `uri` — 待播放的媒体 URI / Media URI to play.
///
/// # 返回值 / Returns
/// - 非网络 URI 或已确认时返回 `Ok(())` / `Ok(())` for non-network URIs or when
///   already confirmed.
///
/// # 错误 / Errors
/// - 当前实现不返回错误（假定 `GStreamerInitProvider` 已在启动时初始化）。
///   Current implementation does not return errors (assumes `GStreamerInitProvider`
///   initialized at startup).
///
/// # 平台 / Platform
/// - 仅 **Android** / **Android** only.
#[cfg(target_os = "android")]
pub fn ensure_java_gstreamer_for_network(uri: &str) -> Result<()> {
    let trimmed = uri.trim();
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Ok(());
    }
    if JAVA_GST_CONTEXT_ENSURED.load(std::sync::atomic::Ordering::SeqCst) {
        return Ok(());
    }
    JAVA_GST_CONTEXT_ENSURED.store(true, std::sync::atomic::Ordering::SeqCst);
    crate::diag::logcat_info("gst: network URI — GStreamer.init assumed via GStreamerInitProvider");
    Ok(())
}

/// Eagerly initializes `reqwesthttpsrc`'s embedded Tokio runtime from
/// [`GStreamerInitProvider`] (before first `playbin` preroll).
#[cfg(target_os = "android")]
pub fn warmup_reqwest_httpsrc_runtime() {
    use gstreamer::prelude::*;

    if REQWEST_RUNTIME_WARMED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }

    if let Err(e) = ensure_gst_init_android() {
        crate::diag::logcat_error(&format!("gst: reqwest warmup gst init failed: {e:#}"));
        return;
    }

    ensure_android_http_uri_handler();

    let src = match gst::ElementFactory::make("reqwesthttpsrc")
        .name("xhvp-reqwest-warmup")
        .build()
    {
        Ok(element) => element,
        Err(e) => {
            crate::diag::logcat_error(&format!("gst: reqwest warmup element create failed: {e}"));
            return;
        }
    };

    src.set_property("location", REQWEST_WARMUP_URL);

    let pipeline = gst::Pipeline::new();
    if pipeline.add(&src).is_err() {
        crate::diag::logcat_error("gst: reqwest warmup pipeline add src failed");
        return;
    }

    let sink = match gst::ElementFactory::make("fakesink").build() {
        Ok(element) => element,
        Err(e) => {
            crate::diag::logcat_error(&format!("gst: reqwest warmup fakesink failed: {e}"));
            return;
        }
    };
    if pipeline.add(&sink).is_err() {
        crate::diag::logcat_error("gst: reqwest warmup pipeline add sink failed");
        return;
    }

    if gst::Element::link_many([&src, &sink]).is_err() {
        crate::diag::logcat_error("gst: reqwest warmup link failed");
        return;
    }

    match pipeline.set_state(gst::State::Paused) {
        Ok(_) => {
            crate::diag::logcat_info("gst: reqwesthttpsrc RUNTIME warmed up (Paused probe)");
        }
        Err(e) => {
            crate::diag::logcat_error(&format!(
                "gst: reqwest warmup Paused failed (RUNTIME may still be init): {e:#}"
            ));
        }
    }
    let _ = pipeline.set_state(gst::State::Null);
}

/// Eagerly creates the process-wide GstGL display (`gldisplay-event` thread) from
/// [`GStreamerInitProvider`] while Bionic pthread keys are still available.
///
/// `glimagesink` lazily starts this thread on first HW-decode preroll; on SDK-heavy
/// apps that can exhaust pthread keys and SIGABRT in `g_private_get`.
#[cfg(target_os = "android")]
pub fn warmup_gst_gl_display() {
    use gstreamer::prelude::*;
    use std::sync::atomic::Ordering;

    if GL_DISPLAY_WARMED.swap(true, Ordering::SeqCst) {
        return;
    }

    super::spawn_on_gst_thread(|| {
        if let Err(e) = ensure_gst_init_android() {
            crate::diag::logcat_error(&format!("gst: GL display warmup gst init failed: {e:#}"));
            return;
        }

        let sink = match gst::ElementFactory::make("glimagesink")
            .name("xhvp-gl-warmup")
            .build()
        {
            Ok(element) => element,
            Err(e) => {
                crate::diag::logcat_error(&format!(
                    "gst: GL display warmup glimagesink create failed: {e}"
                ));
                return;
            }
        };

        match sink.set_state(gst::State::Ready) {
            Ok(_) => {
                let _ = sink.set_state(gst::State::Null);
                crate::diag::logcat_info(
                    "gst: GstGL display warmed (gldisplay-event claimed at process start)",
                );
            }
            Err(e) => {
                crate::diag::logcat_error(&format!("gst: GL display warmup Ready failed: {e:#}"));
            }
        }
    });
}
