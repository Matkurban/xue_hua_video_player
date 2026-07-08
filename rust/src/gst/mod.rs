//! GStreamer process bootstrap and dedicated `xhvp-gst` runtime thread.

mod android;
mod env;
mod init;
#[cfg(target_os = "ios")]
mod ios_plugins;
mod runtime;
mod tls;
#[cfg(target_os = "macos")]
mod tls_macos;

#[cfg(target_os = "android")]
pub use android::ensure_gst_init_android;
#[cfg(target_os = "android")]
pub use android::ensure_java_gstreamer_for_network;
pub use init::ensure_gst_init;
pub use runtime::{
    ensure_gst_runtime, gst_main_context, run_on_gst_thread, spawn_on_gst_thread,
    spawn_on_gst_thread_and_wait,
};
