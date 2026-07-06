//! Android GStreamer init when [`GStreamerInitProvider`] has already called
//! `GStreamer.init(context)` at process startup.

use anyhow::{anyhow, Result};
use gstreamer as gst;
#[cfg(target_os = "android")]
use irondash_run_loop::RunLoop;

static JAVA_GST_CONTEXT_ENSURED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Syncs gstreamer-rs with the C runtime (Java provider or `gst::init`).
pub fn ensure_gst_init_android() -> Result<()> {
    crate::diag::logcat_info("gst: ensure_gst_init_android enter");

    #[cfg(target_os = "android")]
    crate::android_gst_runtime::ensure_gst_runtime();

    let c_initialized = unsafe {
        gst::ffi::gst_is_initialized() != gst::glib::ffi::GFALSE
    };
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
/// ran with the Activity `Context` (Provider uses application context).
#[cfg(target_os = "android")]
pub fn ensure_java_gstreamer_for_network(engine_handle: i64, uri: &str) -> Result<()> {
    let trimmed = uri.trim();
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Ok(());
    }
    if JAVA_GST_CONTEXT_ENSURED.load(std::sync::atomic::Ordering::SeqCst) {
        return Ok(());
    }

    let sender = RunLoop::sender_for_main_thread()
        .map_err(|e| anyhow!("cannot reach main thread for GStreamer.init: {e:?}"))?;
    ensure_java_gstreamer_on_main(engine_handle, &sender)?;
    JAVA_GST_CONTEXT_ENSURED.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[cfg(target_os = "android")]
fn ensure_java_gstreamer_on_main(
    engine_handle: i64,
    sender: &irondash_run_loop::RunLoopSender,
) -> Result<()> {
    sender.send_and_wait(move || -> Result<()> {
        use irondash_engine_context::EngineContext;
        use jni::{jni_sig, jni_str, Env};

        let engine = EngineContext::get().map_err(|e| anyhow!("EngineContext: {e:?}"))?;
        let activity = engine.get_activity(engine_handle)?;
        let vm = EngineContext::get_java_vm()?;

        vm.attach_current_thread(|env: &mut Env<'_>| -> Result<()> {
            let class = env.find_class(jni_str!("org/freedesktop/gstreamer/GStreamer"))?;
            env.call_static_method(
                class,
                jni_str!("init"),
                jni_sig!("(Landroid/content/Context;)V"),
                &[activity.as_obj().into()],
            )?;
            if env.exception_check() {
                env.exception_describe();
                env.exception_clear();
                return Err(anyhow!("GStreamer.init(Context) raised a Java exception"));
            }
            Ok(())
        })?;

        crate::diag::logcat_info("gst: GStreamer.init(Context) ensured for network playback");
        Ok(())
    })?;
    Ok(())
}
