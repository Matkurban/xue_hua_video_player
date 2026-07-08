//! Android GStreamer init when [`GStreamerInitProvider`] has already called
//! `GStreamer.init(context)` at process startup.

use anyhow::{anyhow, Result};
use gstreamer as gst;

static JAVA_GST_CONTEXT_ENSURED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Syncs gstreamer-rs with the C runtime (Java provider or `gst::init`).
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

/// Before the first HTTP(S) `playbin` URI, ensure Java `GStreamer.init(Context)`
/// ran. `GStreamerInitProvider` already does this at process startup.
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
