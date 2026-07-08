//! GIO TLS 后端注册，使 `souphttpsrc` 能拉取 `https://` 资源。
//!
//! GIO TLS backend registration so `souphttpsrc` can fetch `https://` URIs.
//!
//! iOS 使用静态链接框架中的符号；macOS 委托给 [`super::tls_macos`] 动态加载模块。

#[cfg(target_os = "ios")]
extern "C" {
    fn _g_io_modules_ensure_extension_points_registered();
    fn g_io_openssl_load(module: *mut std::ffi::c_void);
}

#[cfg(target_os = "ios")]
#[link(name = "dl")]
extern "C" {
    fn dlsym(handle: *mut std::ffi::c_void, symbol: *const i8) -> *mut std::ffi::c_void;
}

#[cfg(target_os = "ios")]
const RTLD_DEFAULT: *mut std::ffi::c_void = -2isize as *mut std::ffi::c_void;

/// 注册 GIO TLS 后端，使 `souphttpsrc` 能获取 `https://` URI。
/// Registers a GIO TLS backend so `souphttpsrc` can fetch `https://` URIs.
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 平台 / Platform
/// - **iOS**：注册 OpenSSL，并尝试可选的 DarwinSSL 模块。
///   Registers OpenSSL and attempts the optional DarwinSSL module.
#[cfg(target_os = "ios")]
pub fn register_gio_tls_backend() {
    unsafe {
        _g_io_modules_ensure_extension_points_registered();
        g_io_openssl_load(std::ptr::null_mut());
    }
    let mut backends = vec!["OpenSSL"];
    if try_optional_gio_module_load("g_io_darwinssl_load") {
        backends.push("DarwinSSL");
    }
    log::info!(
        "gst: iOS GIO TLS backends registered ({})",
        backends.join(", ")
    );
}

/// 从静态链接的 GStreamer 框架中解析并加载可选 GIO TLS 模块符号。
/// Loads an optional GIO TLS module symbol from the statically linked GStreamer framework.
///
/// # 参数 / Parameters
/// - `symbol` — C 入口符号名（如 `g_io_darwinssl_load`）/ C entry symbol name
///   (e.g. `g_io_darwinssl_load`).
///
/// # 返回值 / Returns
/// - 符号存在且加载成功时返回 `true` / `true` if the symbol exists and loads successfully.
#[cfg(target_os = "ios")]
fn try_optional_gio_module_load(symbol: &str) -> bool {
    let Ok(sym_c) = std::ffi::CString::new(symbol) else {
        return false;
    };
    // SAFETY: resolves optional TLS module entry points from the app/GStreamer image.
    unsafe {
        let sym = dlsym(RTLD_DEFAULT, sym_c.as_ptr());
        if sym.is_null() {
            return false;
        }
        let load: extern "C" fn(*mut std::ffi::c_void) = std::mem::transmute(sym);
        load(std::ptr::null_mut());
        true
    }
}

/// 注册 macOS GIO TLS 后端（委托给 [`super::tls_macos`]）。
/// Registers the macOS GIO TLS backend (delegates to [`super::tls_macos`]).
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 平台 / Platform
/// - 仅 **macOS** / **macOS** only.
#[cfg(target_os = "macos")]
pub fn register_gio_tls_backend() {
    super::tls_macos::register_gio_tls_backend(super::env::bundled_gstreamer_lib_dir().as_deref());
}
