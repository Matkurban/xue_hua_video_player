#[cfg(target_os = "ios")]
extern "C" {
    fn _g_io_modules_ensure_extension_points_registered();
    fn g_io_openssl_load(module: *mut std::ffi::c_void);
}

/// Registers the OpenSSL-based GIO TLS backend so `https://` sources work.
#[cfg(target_os = "ios")]
pub fn register_gio_tls_backend() {
    unsafe {
        _g_io_modules_ensure_extension_points_registered();
        g_io_openssl_load(std::ptr::null_mut());
    }
}

#[cfg(target_os = "macos")]
pub fn register_gio_tls_backend() {
    crate::macos_gio_tls::register_gio_tls_backend(
        super::env::bundled_gstreamer_lib_dir().as_deref(),
    );
}
