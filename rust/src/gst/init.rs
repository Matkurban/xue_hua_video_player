//! GStreamer 一次性进程级初始化入口。
//!
//! One-shot, process-wide GStreamer initialization entry point.
//!
//! 协调平台环境设置（[`super::env`]）、[`super::runtime`] 线程启动、
//! Android Java 同步、iOS 静态插件与 TLS 注册。由 [`ensure_gst_init`] 保证
//! 整个进程只执行一次。

use anyhow::{anyhow, Result};

use super::runtime::run_on_gst_thread;

/// 确保 `gst::init()` 在整个进程中恰好执行一次。
/// Ensures `gst::init()` runs exactly once for the process.
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// - 成功时返回 `Ok(())` / `Ok(())` on success.
///
/// # 错误 / Errors
/// - 平台环境设置、Android 初始化或 Gst 线程上的插件/TLS 注册失败时返回错误。
///   Returns an error if platform env setup, Android init, or plugin/TLS
///   registration on the Gst thread fails.
///
/// # 线程 / Threading
/// - 非 Android 平台：iOS/macOS 的插件与 TLS 注册在 `xhvp-gst` 线程上执行。
///   On non-Android platforms, iOS/macOS plugin and TLS registration runs on
///   the `xhvp-gst` thread.
/// - Android：`gst::init()` 在调用线程执行，但会先启动 `xhvp-gst` 运行时。
///   On Android, `gst::init()` runs on the calling thread after starting the
///   `xhvp-gst` runtime.
///
/// # 平台 / Platform
/// - **iOS**：[`super::env::setup_ios_env`] + 静态插件 + TLS。
/// - **macOS**：[`super::env::setup_macos_env`] + TLS。
/// - **Android**：[`super::android::ensure_gst_init_android`]。
pub fn ensure_gst_init() -> Result<()> {
    use std::sync::Once;
    static INIT: Once = Once::new();
    static mut RESULT: Option<Result<()>> = None;
    // SAFETY: guarded by Once, only written inside call_once.
    unsafe {
        INIT.call_once(|| {
            RESULT = Some((|| {
                #[cfg(target_os = "ios")]
                super::env::setup_ios_env();
                #[cfg(target_os = "macos")]
                super::env::setup_macos_env();
                super::runtime::ensure_gst_runtime();
                #[cfg(target_os = "android")]
                {
                    super::android::ensure_gst_init_android()?;
                }
                #[cfg(not(target_os = "android"))]
                {
                    run_on_gst_thread(|| {
                        #[cfg(target_os = "ios")]
                        {
                            super::ios_plugins::register_ios_static_plugins();
                            super::tls::register_gio_tls_backend();
                        }
                        #[cfg(target_os = "macos")]
                        super::tls::register_gio_tls_backend();
                        Ok(())
                    })?;
                }
                Ok(())
            })());
        });
        match &*std::ptr::addr_of!(RESULT) {
            Some(Ok(())) => Ok(()),
            Some(Err(e)) => Err(anyhow!("{e}")),
            None => Err(anyhow!("gst init state missing")),
        }
    }
}
