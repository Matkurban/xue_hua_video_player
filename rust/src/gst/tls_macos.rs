//! macOS 上通过 `dlopen` 加载捆绑的 GIO OpenSSL TLS 模块。
//!
//! Loads the bundled GIO OpenSSL TLS module via `dlopen` on macOS.
//!
//! 与 iOS 静态框架不同，macOS GStreamer 将 TLS 模块作为 `.so`/`.dylib` 置于
//! `lib/gio/modules/`，需在运行时显式加载（除非 `GIO_MODULE_DIR` 已由 GLib 处理）。

use std::ffi::CString;
use std::path::{Path, PathBuf};

#[link(name = "dl")]
extern "C" {
    fn dlopen(filename: *const i8, flag: i32) -> *mut std::ffi::c_void;
    fn dlsym(handle: *mut std::ffi::c_void, symbol: *const i8) -> *mut std::ffi::c_void;
}

const RTLD_LAZY: i32 = 0x1;
const RTLD_LOCAL: i32 = 0x4;

/// 加载捆绑的 GIO OpenSSL TLS 模块，使 `souphttpsrc` 能获取 `https://` URI。
/// Loads the bundled GIO OpenSSL TLS module so `souphttpsrc` can fetch `https://` URIs.
///
/// # 参数 / Parameters
/// - `lib_dir` — GStreamer `lib` 目录（来自 [`super::env::bundled_gstreamer_lib_dir`]）；
///   为 `None` 时仅依赖 `GIO_MODULE_DIR` 环境变量。
///   GStreamer `lib` directory (from [`super::env::bundled_gstreamer_lib_dir`]); when
///   `None`, relies only on the `GIO_MODULE_DIR` environment variable.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 平台 / Platform
/// - 仅 **macOS** / **macOS** only.
/// - 若 `setup_macos_env()` 已设置 `GIO_MODULE_DIR`，则跳过手动 `dlopen` 以避免重复注册。
///   Skips manual `dlopen` when `GIO_MODULE_DIR` is set to avoid duplicate registration.
pub fn register_gio_tls_backend(lib_dir: Option<&Path>) {
    // `setup_macos_env()` sets `GIO_MODULE_DIR` before `gst::init()`; GLib loads GIO
    // modules from that directory automatically. Manual `dlopen` duplicates registration
    // and breaks the TLS backend (`GTlsBackendOpenssl` registered twice).
    if std::env::var("GIO_MODULE_DIR").is_ok() {
        log::info!("macOS: GIO_MODULE_DIR set; TLS modules loaded by GLib");
        return;
    }

    let mut candidates = Vec::new();
    if let Some(dir) = lib_dir {
        candidates.extend(module_paths_in(&dir.join("gio").join("modules")));
    }

    let candidate_count = candidates.len();
    for path in candidates {
        if try_load_module(&path) {
            log::info!("macOS: loaded GIO TLS module from {}", path.display());
            return;
        }
    }
    log::error!(
        "macOS: GIO OpenSSL TLS module not found (checked {candidate_count} paths); \
         https:// sources will fail — verify GStreamer.framework/lib/gio/modules/ is bundled"
    );
}

/// 枚举 `gio/modules` 目录下已知的 TLS 模块文件名。
/// Enumerates known TLS module filenames under `gio/modules`.
///
/// # 参数 / Parameters
/// - `dir` — `gio/modules` 目录路径 / Path to the `gio/modules` directory.
///
/// # 返回值 / Returns
/// - 候选模块完整路径列表 / List of candidate module full paths.
fn module_paths_in(dir: &Path) -> Vec<PathBuf> {
    [
        "libgiolibopenssl.so",
        "libgioopenssl.so",
        "libgiolibopenssl.dylib",
        "libgioopenssl.dylib",
    ]
    .into_iter()
    .map(|name| dir.join(name))
    .collect()
}

/// 尝试 `dlopen` 指定路径的 GIO 模块并调用其加载符号。
/// Attempts to `dlopen` the GIO module at `path` and invoke its load symbol.
///
/// # 参数 / Parameters
/// - `path` — 模块文件路径 / Module file path.
///
/// # 返回值 / Returns
/// - 加载成功时返回 `true` / `true` on successful load.
fn try_load_module(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(path_c) = CString::new(path.to_string_lossy().as_ref()) else {
        return false;
    };
    // SAFETY: loading a GIO module from the bundled GStreamer framework path.
    unsafe {
        let handle = dlopen(path_c.as_ptr(), RTLD_LAZY | RTLD_LOCAL);
        if handle.is_null() {
            return false;
        }
        if call_symbol(handle, "g_io_openssl_load") {
            return true;
        }
        call_symbol(handle, "g_io_module_load")
    }
}

/// 在已打开的模块句柄上解析并调用指定加载符号。
/// Resolves and invokes the named load symbol on an opened module handle.
///
/// # 参数 / Parameters
/// - `handle` — `dlopen` 返回的模块句柄 / Module handle returned by `dlopen`.
/// - `name` — 符号名（`g_io_openssl_load` 或 `g_io_module_load`）/ Symbol name.
///
/// # 返回值 / Returns
/// - 符号解析并调用成功时返回 `true` / `true` if the symbol resolves and runs successfully.
///
/// # 错误 / Errors
/// - 无显式错误返回；失败时返回 `false` / No explicit error; returns `false` on failure.
unsafe fn call_symbol(handle: *mut std::ffi::c_void, name: &str) -> bool {
    let Ok(sym_c) = CString::new(name) else {
        return false;
    };
    let sym = dlsym(handle, sym_c.as_ptr());
    if sym.is_null() {
        return false;
    }
    if name == "g_io_openssl_load" {
        let load: extern "C" fn(*mut std::ffi::c_void) = std::mem::transmute(sym);
        load(std::ptr::null_mut());
    } else {
        let load: extern "C" fn() = std::mem::transmute(sym);
        load();
    }
    true
}
