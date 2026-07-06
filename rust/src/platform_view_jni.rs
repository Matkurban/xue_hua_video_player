#[cfg(target_os = "macos")]
use crate::api::player::{apply_macos_overlay_gstreamer, cache_macos_overlay_handle};
#[cfg(not(target_os = "macos"))]
use crate::api::player::set_video_overlay_window;

#[cfg(target_os = "android")]
use jni::objects::JObject;
#[cfg(target_os = "android")]
use jni::JNIEnv;

#[cfg(target_os = "android")]
use crate::platform_view_android::{native_window_handle_from_surface, store_java_vm};

/// C ABI entry for Swift / C++ platform views.
#[no_mangle]
pub extern "C" fn player_set_video_overlay_window(player_id: i64, window_handle: i64) {
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = cache_macos_overlay_handle(player_id, window_handle) {
            log::warn!(
                "player_set_video_overlay_window cache(player_id={player_id}, \
                 handle={window_handle}): {e:#}"
            );
        }
        return;
    }
    #[cfg(not(target_os = "macos"))]
    if let Err(e) = set_video_overlay_window(player_id, window_handle) {
        log::warn!(
            "player_set_video_overlay_window(player_id={player_id}, handle={window_handle}): {e:#}"
        );
    }
}

/// macOS: synchronously records the target `NSView` handle for bus sync / rebind.
#[cfg(target_os = "macos")]
#[no_mangle]
pub extern "C" fn player_sync_macos_video_layer(
    player_id: i64,
    ns_view_ptr: i64,
    _width: i32,
    _height: i32,
) {
    if let Err(e) = cache_macos_overlay_handle(player_id, ns_view_ptr) {
        log::warn!(
            "player_sync_macos_video_layer cache(player_id={player_id}, view={ns_view_ptr}): {e:#}"
        );
    }
}

/// macOS: binds the cached `NSView` to the GStreamer sink. Must run on the main thread.
#[cfg(target_os = "macos")]
#[no_mangle]
pub extern "C" fn player_apply_macos_overlay_gstreamer(
    player_id: i64,
    width: i32,
    height: i32,
) {
    if let Err(e) = apply_macos_overlay_gstreamer(player_id, width, height) {
        log::warn!(
            "player_apply_macos_overlay_gstreamer(player_id={player_id}, \
             {width}x{height}): {e:#}"
        );
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_flutter_rust_bridge_xue_hua_video_player_XueHuaVideoPlatformView_nativeOnSurfaceCreated(
    mut env: JNIEnv,
    _class: jni::objects::JClass,
    player_id: i64,
    surface: JObject,
) {
    let _ = on_android_surface(player_id, &mut env, surface);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_flutter_rust_bridge_xue_hua_video_player_XueHuaVideoPlatformView_nativeOnSurfaceChanged(
    mut env: JNIEnv,
    _class: jni::objects::JClass,
    player_id: i64,
    surface: JObject,
) {
    let _ = on_android_surface(player_id, &mut env, surface);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_flutter_rust_bridge_xue_hua_video_player_XueHuaVideoPlatformView_nativeOnSurfaceDestroyed(
    _env: JNIEnv,
    _class: jni::objects::JClass,
    player_id: i64,
) {
    let _ = set_video_overlay_window(player_id, 0);
}

#[cfg(target_os = "android")]
fn on_android_surface(
    player_id: i64,
    env: &mut JNIEnv,
    surface: JObject,
) -> anyhow::Result<()> {
    if let Ok(vm) = env.get_java_vm() {
        store_java_vm(vm.as_raw() as *mut jni::sys::JavaVM);
    }
    let handle = native_window_handle_from_surface(env, surface)? as i64;
    set_video_overlay_window(player_id, handle)
}
