//! macOS main-thread dispatch for VideoOverlay apply (`osxvideosink` requires main thread).

use std::ffi::c_void;

struct MainThreadWork(Box<dyn FnOnce() + Send>);

extern "C" fn main_thread_trampoline(ctx: *mut c_void) {
    // SAFETY: pointer came from `Box::into_raw` in `run_on_main`.
    let work = unsafe { Box::from_raw(ctx as *mut MainThreadWork) };
    (work.0)();
}

extern "C" {
    fn dispatch_get_main_queue() -> *mut c_void;
    fn dispatch_async_f(queue: *mut c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}

/// Schedules `f` on the main queue without blocking the caller.
pub fn run_on_main<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let ctx = Box::into_raw(Box::new(MainThreadWork(Box::new(f))));
    unsafe {
        dispatch_async_f(
            dispatch_get_main_queue(),
            ctx as *mut c_void,
            main_thread_trampoline,
        );
    }
}
