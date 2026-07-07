use std::sync::Once;

static ANDROID_DIAG_INIT: Once = Once::new();

/// Direct logcat output that does not depend on the global `log` logger (host SDKs
/// such as `xue_hua_sdk` may own it).
#[cfg(target_os = "android")]
mod logcat {
    use std::ffi::CString;

    const ANDROID_LOG_INFO: i32 = 4;
    const ANDROID_LOG_ERROR: i32 = 6;

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(
            prio: i32,
            tag: *const std::ffi::c_char,
            text: *const std::ffi::c_char,
        ) -> i32;
    }

    fn write(prio: i32, msg: &str) {
        let tag = CString::new("xue_hua_video_player").expect("tag");
        let bytes = msg.as_bytes();
        let len = bytes.len().min(3500);
        let text = CString::new(&bytes[..len]).unwrap_or_else(|_| CString::new("…").unwrap());
        unsafe {
            __android_log_write(prio, tag.as_ptr(), text.as_ptr());
        }
    }

    pub fn info(msg: &str) {
        write(ANDROID_LOG_INFO, msg);
    }

    pub fn error(msg: &str) {
        write(ANDROID_LOG_ERROR, msg);
    }
}

#[cfg(target_os = "android")]
pub fn logcat_info(msg: &str) {
    logcat::info(msg);
}

#[cfg(target_os = "android")]
pub fn logcat_error(msg: &str) {
    logcat::error(msg);
}

#[cfg(not(target_os = "android"))]
pub fn logcat_info(_msg: &str) {}

#[cfg(not(target_os = "android"))]
pub fn logcat_error(_msg: &str) {}

/// Installs a panic hook that writes to logcat via `__android_log_write`.
#[cfg(target_os = "android")]
pub fn ensure_android_diagnostics_initialized() {
    ANDROID_DIAG_INIT.call_once(|| {
        logcat::info("diag: panic hook installing");
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let backtrace = std::backtrace::Backtrace::force_capture();
            logcat::error(&format!("PANIC: {info}\n{backtrace:?}"));
            default_hook(info);
        }));
        logcat::info("diag: panic hook installed");
    });
}
