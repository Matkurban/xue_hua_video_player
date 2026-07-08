//! Native platform FFI: JNI, Android Surface, iOS UIKit/CALayer bridges.

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "ios")]
pub mod ios;
pub mod jni;
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "android")]
pub use android::{
    attach_java_vm, bind_flutter_asset_helper_class, call_open_asset_fd,
    native_window_handle_from_surface, release_native_window, store_java_vm, with_jni_env,
};
#[cfg(target_os = "ios")]
pub use ios::{
    attach_layer_on_main_thread_async, attach_layer_on_main_thread_sync, host_view_ready_for_attach,
};
#[cfg(target_os = "macos")]
pub use macos::{run_on_main, run_on_main_sync};
