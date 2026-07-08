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
