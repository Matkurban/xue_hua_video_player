#[cfg(target_os = "ios")]
use crate::api::player::apply_ios_overlay_gstreamer;
#[cfg(target_os = "macos")]
use crate::api::player::apply_macos_overlay_gstreamer;
#[cfg(target_os = "macos")]
use crate::api::player::cache_macos_overlay_handle;
#[cfg(target_os = "android")]
use crate::api::player::notify_android_surface;
#[cfg(target_os = "ios")]
use crate::api::player::notify_ios_overlay;
#[cfg(all(
    not(target_os = "macos"),
    not(target_os = "android"),
    not(target_os = "ios")
))]
use crate::api::player::set_video_overlay_window;
#[cfg(not(any(target_os = "macos", target_os = "android")))]
use crate::api::player::sync_video_overlay_rectangle;

#[cfg(target_os = "android")]
use jni::errors::LogErrorAndDefault;
#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject};
#[cfg(target_os = "android")]
use jni::{jni_mangle, Env, EnvUnowned};

#[cfg(target_os = "android")]
use crate::platform::android::{native_window_handle_from_surface, store_java_vm};

/// Caches the process JavaVM when the native library loads (before Platform View surface).
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn JNI_OnLoad(
    vm: *mut jni::sys::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> jni::sys::jint {
    // SAFETY: JNI passes a valid JavaVM pointer during library load.
    let java_vm = unsafe { jni::JavaVM::from_raw(vm) };
    store_java_vm(java_vm.get_raw());
    crate::diag::logcat_info("JNI_OnLoad: JavaVM cached for Rust JNI calls");
    jni::sys::JNI_VERSION_1_6
}

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
    #[cfg(target_os = "ios")]
    {
        if let Err(e) = notify_ios_overlay(player_id, window_handle, 0, 0) {
            log::warn!(
                "player_set_video_overlay_window(player_id={player_id}, \
                 handle={window_handle}): {e:#}"
            );
        }
        return;
    }
    #[cfg(target_os = "android")]
    {
        if let Err(e) = notify_android_surface(player_id, window_handle, 0, 0) {
            log::warn!(
                "player_set_video_overlay_window(player_id={player_id}, \
                 handle={window_handle}): {e:#}"
            );
        }
        return;
    }
    #[cfg(all(
        not(target_os = "macos"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    if let Err(e) = set_video_overlay_window(player_id, window_handle) {
        log::warn!(
            "player_set_video_overlay_window(player_id={player_id}, handle={window_handle}): {e:#}"
        );
    }
}

/// C ABI: sync VideoOverlay render rectangle after native view resize (desktop).
#[no_mangle]
pub extern "C" fn player_sync_video_overlay_rectangle(player_id: i64, width: i32, height: i32) {
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = apply_macos_overlay_gstreamer(player_id, width, height) {
            log::warn!(
                "player_sync_video_overlay_rectangle(player_id={player_id}, \
                 {width}x{height}): {e:#}"
            );
        }
        return;
    }
    #[cfg(target_os = "android")]
    {
        let _ = (player_id, width, height);
        return;
    }
    #[cfg(not(any(target_os = "macos", target_os = "android", target_os = "ios")))]
    if let Err(e) = sync_video_overlay_rectangle(player_id, width, height) {
        log::warn!(
            "player_sync_video_overlay_rectangle(player_id={player_id}, \
             {width}x{height}): {e:#}"
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
pub extern "C" fn player_apply_macos_overlay_gstreamer(player_id: i64, width: i32, height: i32) {
    if let Err(e) = apply_macos_overlay_gstreamer(player_id, width, height) {
        log::warn!(
            "player_apply_macos_overlay_gstreamer(player_id={player_id}, \
             {width}x{height}): {e:#}"
        );
    }
}

/// iOS: attaches `avsamplebufferlayersink` CALayer and prerolls. Must run on the main thread.
#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn player_apply_ios_overlay_gstreamer(player_id: i64, width: i32, height: i32) {
    if let Err(e) = apply_ios_overlay_gstreamer(player_id, width, height) {
        log::warn!(
            "player_apply_ios_overlay_gstreamer(player_id={player_id}, \
             {width}x{height}): {e:#}"
        );
    }
}

/// iOS: records the Flutter assets directory before the first asset load.
#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn xhvp_set_flutter_assets_dir(path: *const std::ffi::c_char) {
    if path.is_null() {
        return;
    }
    // SAFETY: Swift passes a NUL-terminated UTF-8 path from Bundle.main.
    let c_str = unsafe { std::ffi::CStr::from_ptr(path) };
    if let Ok(dir) = c_str.to_str() {
        crate::media::set_flutter_assets_dir(dir);
    }
}

/// iOS: caches host `UIView` handle and dimensions; Gst attach runs via `IosOverlaySession`.
#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn player_notify_ios_overlay(
    player_id: i64,
    window_handle: i64,
    width: i32,
    height: i32,
) {
    if let Err(e) = notify_ios_overlay(player_id, window_handle, width, height) {
        log::warn!(
            "player_notify_ios_overlay(player_id={player_id}, handle={window_handle}, \
             {width}x{height}): {e:#}"
        );
    }
}

#[cfg(target_os = "android")]
#[jni_mangle(
    "com.flutter_rust_bridge.xue_hua_video_player.AndroidSurfaceBridge",
    "nativeOnSurfaceCreated",
    "(JLandroid/view/Surface;)V"
)]
pub extern "system" fn native_on_surface_created<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    player_id: i64,
    surface: JObject<'caller>,
) {
    env.with_env(|env| {
        if let Err(e) = on_android_surface(player_id, env, surface, 0, 0) {
            log::warn!("nativeOnSurfaceCreated(player_id={player_id}): {e:#}");
        }
        Ok::<(), jni::errors::Error>(())
    })
    .resolve::<LogErrorAndDefault>();
}

#[cfg(target_os = "android")]
#[jni_mangle(
    "com.flutter_rust_bridge.xue_hua_video_player.AndroidSurfaceBridge",
    "nativeOnSurfaceChanged",
    "(JLandroid/view/Surface;II)V"
)]
pub extern "system" fn native_on_surface_changed<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    player_id: i64,
    surface: JObject<'caller>,
    width: i32,
    height: i32,
) {
    env.with_env(|env| {
        if let Err(e) = on_android_surface(player_id, env, surface, width, height) {
            log::warn!("nativeOnSurfaceChanged(player_id={player_id}): {e:#}");
        }
        Ok::<(), jni::errors::Error>(())
    })
    .resolve::<LogErrorAndDefault>();
}

#[cfg(target_os = "android")]
#[jni_mangle(
    "com.flutter_rust_bridge.xue_hua_video_player.AndroidSurfaceBridge",
    "nativeOnSurfaceDestroyed",
    "(J)V"
)]
pub extern "system" fn native_on_surface_destroyed<'caller>(
    _env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    player_id: i64,
) {
    if let Err(e) = notify_android_surface(player_id, 0, 0, 0) {
        log::warn!("nativeOnSurfaceDestroyed(player_id={player_id}): {e:#}");
    }
}

#[cfg(target_os = "android")]
fn on_android_surface(
    player_id: i64,
    env: &mut Env,
    surface: JObject,
    width: i32,
    height: i32,
) -> anyhow::Result<()> {
    if let Ok(vm) = env.get_java_vm() {
        store_java_vm(vm.get_raw());
    }
    let handle = native_window_handle_from_surface(env, surface)? as i64;
    crate::diag::logcat_info(&format!(
        "gst: android surface callback player_id={player_id} handle={handle:#x} {width}x{height}"
    ));
    notify_android_surface(player_id, handle, width, height)
}

#[cfg(target_os = "android")]
#[jni_mangle(
    "com.flutter_rust_bridge.xue_hua_video_player.FlutterAssetHelper",
    "nativeBindAssetHelperClass",
    "()V"
)]
pub extern "system" fn native_bind_asset_helper_class<'caller>(
    mut env: EnvUnowned<'caller>,
    class: JClass<'caller>,
) {
    env.with_env(|env| {
        crate::platform::android::bind_flutter_asset_helper_class(env, class);
        Ok::<(), jni::errors::Error>(())
    })
    .resolve::<LogErrorAndDefault>();
}

#[cfg(target_os = "android")]
#[jni_mangle(
    "com.flutter_rust_bridge.xue_hua_video_player.XueHuaVideoPlayerPlugin",
    "nativeBindPluginClass",
    "()V"
)]
pub extern "system" fn native_bind_plugin_class<'caller>(
    mut env: EnvUnowned<'caller>,
    class: JClass<'caller>,
) {
    env.with_env(|env| {
        crate::platform::android::bind_xue_hua_video_player_plugin_class(env, class);
        Ok::<(), jni::errors::Error>(())
    })
    .resolve::<LogErrorAndDefault>();
}
