//! GLib `MainLoop` backend for non-Android platforms.

use std::sync::mpsc;
use std::sync::Once;
use std::thread::ThreadId;
use std::time::Duration;

use anyhow::{anyhow, Result};
use gstreamer::glib::source::{self, Priority};
use gstreamer::glib::{self, ControlFlow, MainContext};
use once_cell::sync::OnceCell;

static RUNTIME_STARTED: Once = Once::new();
static GST_CONTEXT: OnceCell<MainContext> = OnceCell::new();
static GST_THREAD_ID: OnceCell<ThreadId> = OnceCell::new();

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
    let _ = GST_THREAD_ID.set(std::thread::current().id());

    let already_initialized =
        unsafe { gstreamer::ffi::gst_is_initialized() != gstreamer::glib::ffi::GFALSE };
    if !already_initialized {
        if let Err(e) = gstreamer::init() {
            crate::diag::logcat_error(&format!(
                "gst: gst::init() on gst thread failed: {e} — continuing anyway"
            ));
        }
    }

    let context = MainContext::new();
    let _ = GST_CONTEXT.set(context.clone());
    let _ = ready_tx.send(());

    let main_loop = glib::MainLoop::new(Some(&context), false);
    main_loop.run();
}

pub fn gst_main_context() -> Result<&'static MainContext> {
    ensure_gst_runtime();
    GST_CONTEXT
        .get()
        .ok_or_else(|| anyhow!("Gst runtime context not ready"))
}

fn gst_context() -> Result<&'static MainContext> {
    gst_main_context()
}

fn dispatch_on_gst_context<F>(ctx: &MainContext, f: F)
where
    F: FnOnce() + Send + 'static,
{
    let mut f = Some(f);
    source::idle_source_new(
        Some("xhvp-dispatch"),
        Priority::DEFAULT,
        move || {
            if let Some(f) = f.take() {
                f();
            }
            ControlFlow::Break
        },
    )
    .attach(Some(ctx));
}

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
    dispatch_on_gst_context(&ctx, f);
}

pub fn spawn_on_gst_thread_and_wait<F, R>(f: F) -> Result<R>
where
    F: FnOnce() -> Result<R> + Send + 'static,
    R: Send + 'static,
{
    if GST_THREAD_ID
        .get()
        .is_some_and(|id| *id == std::thread::current().id())
    {
        return f();
    }
    let ctx = gst_context()?.clone();
    if ctx.is_owner() {
        return f();
    }
    let (tx, rx) = mpsc::sync_channel(1);
    dispatch_on_gst_context(&ctx, move || {
        let _ = tx.send(f());
    });
    rx.recv()
        .map_err(|e| anyhow!("Gst thread dispatch dropped: {e}"))?
}

pub fn run_on_gst_thread<F, R>(f: F) -> Result<R>
where
    F: FnOnce() -> Result<R> + Send + 'static,
    R: Send + 'static,
{
    spawn_on_gst_thread_and_wait(f)
}
