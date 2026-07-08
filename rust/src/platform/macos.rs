//! macOS main-thread dispatch for VideoOverlay apply (`osxvideosink` requires main thread).

use std::ffi::c_void;

struct MainThreadWork(Box<dyn FnOnce() + Send>);

extern "C" {
    fn xhvp_macos_dispatch_async_main(work: extern "C" fn(*mut c_void), ctx: *mut c_void);
    fn xhvp_macos_dispatch_sync_main(work: extern "C" fn(*mut c_void), ctx: *mut c_void);
}

extern "C" fn main_thread_trampoline(ctx: *mut c_void) {
    // SAFETY: pointer came from `Box::into_raw` in `run_on_main` / `run_on_main_sync`.
    let work = unsafe { Box::from_raw(ctx as *mut MainThreadWork) };
    (work.0)();
}

/// Schedules `f` on the main queue without blocking the caller.
pub fn run_on_main<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let ctx = Box::into_raw(Box::new(MainThreadWork(Box::new(f))));
    unsafe {
        xhvp_macos_dispatch_async_main(main_thread_trampoline, ctx as *mut c_void);
    }
}

/// Runs `f` on the main queue and blocks until it completes.
///
/// Safe from the Gst thread (`xhvp-gst`); do not call from the Flutter UI thread
/// while holding a lock the main queue needs (deadlock with `osxvideosink`).
pub fn run_on_main_sync<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let ctx = Box::into_raw(Box::new(MainThreadWork(Box::new(f))));
    unsafe {
        xhvp_macos_dispatch_sync_main(main_thread_trampoline, ctx as *mut c_void);
    }
}
