//! 拥有 `GMainContext` 的专用 GStreamer 线程（GStreamer 应用模型）。
//!
//! Dedicated GStreamer thread with an owned `GMainContext` (GStreamer application
//! model).
//!
//! 进程内唯一的 `xhvp-gst` 线程驱动 `MainLoop`；所有管线创建、状态变更与
//! 总线轮询必须在此线程上执行，以避免 GLib 上下文竞争与死锁。

use std::sync::mpsc;
use std::sync::Once;
use std::time::Duration;

use anyhow::{anyhow, Result};
use gstreamer::glib::{self, MainContext};
use once_cell::sync::OnceCell;

static RUNTIME_STARTED: Once = Once::new();
static GST_CONTEXT: OnceCell<MainContext> = OnceCell::new();

/// 启动进程级 `xhvp-gst` 线程（幂等）。
/// Starts the process-wide `xhvp-gst` thread (idempotent).
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 线程 / Threading
/// - 首次调用时生成名为 `xhvp-gst` 的线程并阻塞最多 5 秒等待其就绪。
///   Spawns the `xhvp-gst` thread on first call and blocks up to 5 seconds for
///   readiness.
/// - 后续调用立即返回 / Subsequent calls return immediately.
///
/// # 平台 / Platform
/// - **Android**：线程入口会附加 Java VM（[`crate::platform::android::attach_java_vm`]）。
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

/// `xhvp-gst` 线程主循环：初始化 GStreamer、推送默认 `MainContext` 并运行 `MainLoop`。
/// `xhvp-gst` thread main loop: initializes GStreamer, pushes the default `MainContext`, and runs `MainLoop`.
fn gst_runtime_thread_main(ready_tx: mpsc::SyncSender<()>) {
    #[cfg(target_os = "android")]
    let _ = crate::platform::android::attach_java_vm();

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

/// 返回由 `xhvp-gst` 线程 `MainLoop` 驱动的 `MainContext` 引用。
/// Returns the owned `MainContext` driven by the `xhvp-gst` thread's `MainLoop`.
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// - 静态生命周期的 `MainContext` 引用 / A `&'static MainContext` reference.
///
/// # 错误 / Errors
/// - 运行时线程尚未完成初始化时返回错误 / Returns an error if the runtime thread
///   has not finished initializing.
///
/// # 线程 / Threading
/// - 可在任意线程调用；若需在该上下文上执行工作，请使用调度 API。
///   Callable from any thread; use the dispatch APIs to run work on this context.
pub fn gst_main_context() -> Result<&'static MainContext> {
    ensure_gst_runtime();
    GST_CONTEXT
        .get()
        .ok_or_else(|| anyhow!("Gst runtime context not ready"))
}

/// 内部便捷封装：获取 Gst `MainContext` / Internal helper to obtain the Gst `MainContext`.
fn gst_context() -> Result<&'static MainContext> {
    gst_main_context()
}

/// 在 Gst 线程上调度 `f`（即发即忘）。
/// Schedules `f` on the Gst thread (fire-and-forget).
///
/// # 参数 / Parameters
/// - `f` — 在 `xhvp-gst` 线程上执行的一次性闭包 / One-shot closure to run on the
///   `xhvp-gst` thread.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 错误 / Errors
/// - 上下文未就绪时记录错误并静默返回 / Logs an error and returns silently if the
///   context is not ready.
///
/// # 线程 / Threading
/// - 调用线程不阻塞 / The calling thread does not block.
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

/// 在 Gst 线程上运行 `f` 并阻塞直到完成。
/// Runs `f` on the Gst thread and blocks until it completes.
///
/// # 参数 / Parameters
/// - `f` — 在 Gst 线程上执行并返回 `Result<R>` 的闭包 / Closure that runs on the Gst
///   thread and returns `Result<R>`.
///
/// # 返回值 / Returns
/// - `f` 的返回值 / The value returned by `f`.
///
/// # 错误 / Errors
/// - 上下文未就绪、通道断开或 `f` 返回错误时失败 / Fails if the context is not ready,
///   the channel is dropped, or `f` returns an error.
///
/// # 线程 / Threading
/// - 若已在 `xhvp-gst` 线程（`MainContext::is_owner`），则内联执行 `f`，避免在同一
///   `GMainContext` 上嵌套 `invoke` + `recv` 导致死锁。
///   If already on the `xhvp-gst` thread (`MainContext::is_owner`), runs `f` inline to
///   avoid nested invoke + recv deadlock on the same `GMainContext`.
pub fn spawn_on_gst_thread_and_wait<F, R>(f: F) -> Result<R>
where
    F: FnOnce() -> Result<R> + Send + 'static,
    R: Send + 'static,
{
    let ctx = gst_context()?.clone();
    if ctx.is_owner() {
        return f();
    }
    let (tx, rx) = mpsc::sync_channel(1);
    ctx.invoke(move || {
        let _ = tx.send(f());
    });
    rx.recv()
        .map_err(|e| anyhow!("Gst thread invoke dropped: {e}"))?
}

/// [`spawn_on_gst_thread_and_wait`] 的别名；已在 `xhvp-gst` 线程时内联执行。
/// Alias for [`spawn_on_gst_thread_and_wait`] — runs inline when already on `xhvp-gst`.
///
/// # 参数 / Parameters
/// - `f` — 在 Gst 线程上执行并返回 `Result<R>` 的闭包 / Closure that runs on the Gst
///   thread and returns `Result<R>`.
///
/// # 返回值 / Returns
/// - `f` 的返回值 / The value returned by `f`.
///
/// # 错误 / Errors
/// - 同 [`spawn_on_gst_thread_and_wait`] / Same as [`spawn_on_gst_thread_and_wait`].
///
/// # 线程 / Threading
/// - 同 [`spawn_on_gst_thread_and_wait`] / Same as [`spawn_on_gst_thread_and_wait`].
pub fn run_on_gst_thread<F, R>(f: F) -> Result<R>
where
    F: FnOnce() -> Result<R> + Send + 'static,
    R: Send + 'static,
{
    spawn_on_gst_thread_and_wait(f)
}
