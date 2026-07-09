//! Deep module: Android native runtime bootstrap for Bionic pthread-key budget.
//!
//! Single seam for process-start warmup. Callers only need [`warmup`] /
//! [`ensure_ready_for_network_preroll`]; FRB handler, `xhvp-gst`, GstGL display,
//! and reqwest readiness stay behind this interface.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

use gstreamer as gst;
use gstreamer::prelude::*;

static BOOTSTRAP_ONCE: Once = Once::new();
static NETWORK_PREROLL_READY: AtomicBool = AtomicBool::new(false);

/// Process-start warmup: FRB handler, `xhvp-gst`, GstGL display, reqwest element.
///
/// Idempotent. Invoked from `GStreamerInitProvider` via JNI while pthread keys
/// are still available. Patched `reqwesthttpsrc` also forces Tokio `RUNTIME`
/// during plugin `register` (inside `GStreamer.init`).
pub fn warmup() {
    BOOTSTRAP_ONCE.call_once(|| {
        // Touch FRB handler so its current_thread Tokio runtime allocates early.
        std::hint::black_box(&*crate::api::frb_handler::FLUTTER_RUST_BRIDGE_HANDLER);
        crate::gst::ensure_gst_runtime();
        // Synchronous on xhvp-gst so gldisplay-event claims keys before return.
        warmup_gst_gl_display_sync();
        // Ensure factory is primary + type path exercised after gst init.
        ensure_reqwest_factory_ready();
        NETWORK_PREROLL_READY.store(true, Ordering::SeqCst);
        crate::diag::logcat_info(
            "xhvp: AndroidNativeRuntimeBootstrap warmed (FRB + xhvp-gst + GstGL + reqwest)",
        );
    });
}

/// Ensures network URI preroll may proceed (bootstrap already ran at process start).
pub fn ensure_ready_for_network_preroll() {
    if NETWORK_PREROLL_READY.load(Ordering::SeqCst) {
        return;
    }
    warmup();
}

fn warmup_gst_gl_display_sync() {
    let _ = crate::gst::spawn_on_gst_thread_and_wait(|| {
        if let Err(e) = super::android::ensure_gst_init_android() {
            crate::diag::logcat_error(&format!("gst: GL display warmup gst init failed: {e:#}"));
            return Ok(());
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
                return Ok(());
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
        Ok(())
    });
}

fn ensure_reqwest_factory_ready() {
    let _ = crate::gst::spawn_on_gst_thread_and_wait(|| {
        if let Err(e) = super::android::ensure_gst_init_android() {
            crate::diag::logcat_error(&format!(
                "gst: reqwest factory ensure gst init failed: {e:#}"
            ));
            return Ok(());
        }

        match gst::ElementFactory::make("reqwesthttpsrc")
            .name("xhvp-reqwest-bootstrap")
            .build()
        {
            Ok(src) => {
                let _ = src.set_state(gst::State::Null);
                crate::diag::logcat_info(
                    "gst: reqwesthttpsrc factory ready (RUNTIME forced at plugin register)",
                );
            }
            Err(e) => {
                crate::diag::logcat_error(&format!(
                    "gst: reqwesthttpsrc bootstrap element create failed: {e}"
                ));
            }
        }
        Ok(())
    });
}
