use std::ffi::CString;
use std::path::{Path, PathBuf};

#[link(name = "dl")]
extern "C" {
    fn dlopen(filename: *const i8, flag: i32) -> *mut std::ffi::c_void;
    fn dlsym(handle: *mut std::ffi::c_void, symbol: *const i8) -> *mut std::ffi::c_void;
}

const RTLD_LAZY: i32 = 0x1;
const RTLD_LOCAL: i32 = 0x4;

/// Loads the bundled GIO OpenSSL TLS module so `souphttpsrc` can fetch `https://` URIs.
///
/// Unlike iOS (static framework), macOS GStreamer ships the module as a `.so` under
/// `lib/gio/modules/` and it must be `dlopen`ed at runtime.
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
