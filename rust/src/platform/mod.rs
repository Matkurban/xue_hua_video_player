//! 原生平台 FFI：JNI、Android Surface、iOS UIKit/CALayer 桥接 /
//! Native platform FFI: JNI, Android Surface, iOS UIKit/CALayer bridges.
//!
//! 为播放层提供跨平台的纹理帧回调与资源访问能力。macOS 与 Win/Linux 均经
//! Flutter 外部 Texture 渲染，不再使用 NSView VideoOverlay 主线程 shim。
//!
//! Supplies texture frame callbacks and asset access for the playback layer.
//! macOS and Win/Linux render via Flutter external Texture (no NSView overlay shim).

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "ios")]
pub mod ios;
pub mod jni;
pub mod texture;

#[cfg(target_os = "android")]
pub use android::{
    attach_java_vm, bind_flutter_asset_helper_class, bind_xue_hua_video_player_plugin_class,
    call_open_asset_fd, native_window_handle_from_surface, notify_texture_content_size,
    release_native_window, store_java_vm, with_jni_env,
};
#[cfg(target_os = "ios")]
pub use ios::{
    attach_layer_on_main_thread_async, attach_layer_on_main_thread_sync, host_view_ready_for_attach,
};
