//! Android 诊断：不依赖全局 `log` 日志器的直接 logcat 输出与 panic 钩子。
//!
//! Android diagnostics: direct logcat output and panic hook independent of the global `log` logger.
//!
//! 宿主 SDK（如 `xue_hua_sdk`）可能已占用全局 logger；本模块通过 `__android_log_write`
//! 保证 GStreamer 初始化与 panic 信息仍可见。非 Android 平台提供空实现桩。

use std::sync::Once;

static ANDROID_DIAG_INIT: Once = Once::new();

/// 直接 logcat 输出，不依赖全局 `log` logger（宿主 SDK 可能已占用）。
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

    /// 写入 logcat 消息（自动截断至 3500 字节）/ Writes a logcat message (truncated to 3500 bytes).
    fn write(prio: i32, msg: &str) {
        let tag = CString::new("xue_hua_video_player").expect("tag");
        let bytes = msg.as_bytes();
        let len = bytes.len().min(3500);
        let text = CString::new(&bytes[..len]).unwrap_or_else(|_| CString::new("…").unwrap());
        unsafe {
            __android_log_write(prio, tag.as_ptr(), text.as_ptr());
        }
    }

    /// 以 INFO 级别写入 logcat / Writes at INFO priority.
    pub fn info(msg: &str) {
        write(ANDROID_LOG_INFO, msg);
    }

    /// 以 ERROR 级别写入 logcat / Writes at ERROR priority.
    pub fn error(msg: &str) {
        write(ANDROID_LOG_ERROR, msg);
    }
}

/// 向 Android logcat 写入 INFO 级别消息。
/// Writes an INFO-level message to Android logcat.
///
/// # 参数 / Parameters
/// - `msg` — 日志文本 / Log text.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 平台 / Platform
/// - **Android**：调用 `__android_log_write` / Calls `__android_log_write`.
/// - **其他平台**：空操作 / No-op on other platforms.
#[cfg(target_os = "android")]
pub fn logcat_info(msg: &str) {
    logcat::info(msg);
}

/// 向 Android logcat 写入 ERROR 级别消息。
/// Writes an ERROR-level message to Android logcat.
///
/// # 参数 / Parameters
/// - `msg` — 日志文本 / Log text.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 平台 / Platform
/// - **Android**：调用 `__android_log_write` / Calls `__android_log_write`.
/// - **其他平台**：空操作 / No-op on other platforms.
#[cfg(target_os = "android")]
pub fn logcat_error(msg: &str) {
    logcat::error(msg);
}

/// 非 Android 平台的 `logcat_info` 空桩。
/// No-op stub for `logcat_info` on non-Android platforms.
#[cfg(not(target_os = "android"))]
pub fn logcat_info(_msg: &str) {}

/// 非 Android 平台的 `logcat_error` 空桩。
/// No-op stub for `logcat_error` on non-Android platforms.
#[cfg(not(target_os = "android"))]
pub fn logcat_error(_msg: &str) {}

/// 安装将 panic 信息写入 logcat 的 panic 钩子（通过 `__android_log_write`）。
/// Installs a panic hook that writes to logcat via `__android_log_write`.
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 平台 / Platform
/// - **Android**：幂等安装，保留默认钩子在 logcat 输出后继续调用。
///   Idempotent install; preserves the default hook after logcat output.
/// - **其他平台**：空操作 / No-op on other platforms.
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

/// 非 Android 平台的诊断初始化空桩。
/// No-op stub for diagnostics initialization on non-Android platforms.
#[cfg(not(target_os = "android"))]
pub fn ensure_android_diagnostics_initialized() {}
