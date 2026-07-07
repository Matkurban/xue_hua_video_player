//! Dedicated GStreamer thread with an owned `GMainContext` (GStreamer application
//! model). All pipeline operations must run on this thread.

use std::sync::mpsc;
use std::sync::Once;
use std::time::Duration;

use anyhow::{anyhow, Result};
use gstreamer::glib::{self, MainContext};
use once_cell::sync::OnceCell;

static RUNTIME_STARTED: Once = Once::new();
static GST_CONTEXT: OnceCell<MainContext> = OnceCell::new();

/// Starts the process-wide `xhvp-gst` thread (idempotent).
pub fn ensure_gst_runtime() {
    RUNTIME_STARTED.call_once(|| {
        let (ready_tx, ready_rx) = mpsc::sync_channel::<()>(1);
        std::thread::Builder::new()
            .name("xhvp-gst".into())
            .spawn(move || gst_runtime_thread_main(ready_tx))
            .expect("xhvp-gst thread spawn");

        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(()) => {
                crate::diag::logcat_info("gst: Gst runtime thread started (owned MainContext)")
            }
            Err(e) => {
                crate::diag::logcat_error(&format!("gst: Gst runtime thread failed to start: {e}"))
            }
        }
    });
}

fn gst_runtime_thread_main(ready_tx: mpsc::SyncSender<()>) {
    #[cfg(target_os = "android")]
    let _ = crate::platform_view_android::attach_java_vm();

    if let Err(e) = gstreamer::init() {
        crate::diag::logcat_error(&format!(
            "gst: gst::init() on gst thread failed: {e} — continuing anyway"
        ));
    }

    let context = MainContext::new();
    use glib::translate::ToGlibPtr;
    // SAFETY: `context` outlives this thread; paired with `MainLoop::run` below.
    unsafe {
        glib::ffi::g_main_context_push_thread_default(context.to_glib_none().0);
    }

    let _ = GST_CONTEXT.set(context.clone());
    let _ = ready_tx.send(());

    let main_loop = glib::MainLoop::new(Some(&context), false);
    main_loop.run();
}

/// Returns the owned `MainContext` driven by the `xhvp-gst` thread's `MainLoop`.
pub fn gst_main_context() -> Result<&'static MainContext> {
    ensure_gst_runtime();
    GST_CONTEXT
        .get()
        .ok_or_else(|| anyhow!("Gst runtime context not ready"))
}

fn gst_context() -> Result<&'static MainContext> {
    gst_main_context()
}

/// Schedules `f` on the Gst thread (fire-and-forget).
pub fn spawn_on_gst_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let ctx = match gst_context() {
        Ok(c) => c.clone(),
        Err(e) => {
            crate::diag::logcat_error(&format!("spawn_on_gst_thread: {e}"));
            return;
        }
    };
    ctx.invoke(move || f());
}

/// Runs `f` on the Gst thread and blocks until it completes.
pub fn spawn_on_gst_thread_and_wait<F, R>(f: F) -> Result<R>
where
    F: FnOnce() -> Result<R> + Send + 'static,
    R: Send + 'static,
{
    let ctx = gst_context()?.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    ctx.invoke(move || {
        let _ = tx.send(f());
    });
    rx.recv()
        .map_err(|e| anyhow!("Gst thread invoke dropped: {e}"))?
}
