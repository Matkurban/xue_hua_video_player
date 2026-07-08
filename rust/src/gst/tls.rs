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

/// Registers a GIO TLS backend so `souphttpsrc` can fetch `https://` URIs.
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

/// Loads an optional GIO TLS module symbol from the statically linked GStreamer framework.
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

#[cfg(target_os = "macos")]
pub fn register_gio_tls_backend() {
    super::tls_macos::register_gio_tls_backend(
        super::env::bundled_gstreamer_lib_dir().as_deref(),
    );
}
