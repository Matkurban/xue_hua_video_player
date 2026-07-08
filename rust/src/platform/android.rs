//! Android JNI 与 `ANativeWindow` 辅助函数 / Android JNI and `ANativeWindow` helpers.
//!
//! 缓存 `JavaVM`、绑定 Flutter 插件 jclass、从 `Surface` 获取原生窗口句柄，
//! 以及通过 JNI 回调调整纹理内容尺寸。
//!
//! Caches `JavaVM`, binds Flutter plugin jclasses, obtains native window handles from `Surface`,
//! and resizes texture content via JNI callbacks.

#[cfg(target_os = "android")]
use std::sync::{
    atomic::{AtomicPtr, Ordering},
    Mutex,
};

#[cfg(target_os = "android")]
use anyhow::{anyhow, Result};
#[cfg(target_os = "android")]
use jni::objects::{Global, JClass, JObject};
#[cfg(target_os = "android")]
use jni::Env;

#[cfg(target_os = "android")]
static JAVA_VM: AtomicPtr<jni::sys::JavaVM> = AtomicPtr::new(std::ptr::null_mut());

#[cfg(target_os = "android")]
static FLUTTER_ASSET_HELPER_CLASS: Mutex<Option<Global<JClass>>> = Mutex::new(None);

#[cfg(target_os = "android")]
extern "C" {
    fn ANativeWindow_fromSurface(
        env: *mut jni::sys::JNIEnv,
        surface: jni::sys::jobject,
    ) -> *mut std::ffi::c_void;
    fn ANativeWindow_release(window: *mut std::ffi::c_void);
}

/// 缓存进程 `JavaVM`，供 Gst 线程附着使用 / Caches the process `JavaVM` for attaching on the Gst thread.
///
/// # 参数 / Parameters
/// - `vm` — 来自 JNI 回调的 `JavaVM` 原始指针 / raw `JavaVM` pointer from JNI callback
#[cfg(target_os = "android")]
pub fn store_java_vm(vm: *mut jni::sys::JavaVM) {
    JAVA_VM.store(vm, Ordering::SeqCst);
}

/// 绑定 `FlutterAssetHelper` 全局 jclass / Binds the `FlutterAssetHelper` global jclass.
///
/// # 参数 / Parameters
/// - `env` — JNI 环境 / JNI environment
/// - `class` — `FlutterAssetHelper` 类引用 / `FlutterAssetHelper` class reference
#[cfg(target_os = "android")]
pub fn bind_flutter_asset_helper_class(env: &mut Env, class: JClass) {
    match env.new_global_ref(class) {
        Ok(global) => {
            *FLUTTER_ASSET_HELPER_CLASS.lock().unwrap() = Some(global);
            crate::diag::logcat_info("FlutterAssetHelper jclass bound for Rust JNI");
        }
        Err(e) => {
            crate::diag::logcat_error(&format!("FlutterAssetHelper jclass bind failed: {e}"));
        }
    }
}

/// 使用缓存的 application jclass 调用 `FlutterAssetHelper.openAssetFd` /
/// Invokes `FlutterAssetHelper.openAssetFd` using the cached application jclass.
///
/// # 参数 / Parameters
/// - `env` — JNI 环境 / JNI environment
/// - `asset_key` — Flutter asset 键 / Flutter asset key
///
/// # 返回值 / Returns
/// - `Ok((fd, start, length))` 文件描述符与偏移/长度 / file descriptor with offset/length
///
/// # 错误 / Errors
/// - jclass 未绑定、JNI 调用失败或 fd 不可用 / jclass not bound, JNI failure, or fd unavailable
#[cfg(target_os = "android")]
pub fn call_open_asset_fd(env: &mut Env, asset_key: &str) -> Result<(i32, u64, u64)> {
    use jni::objects::{JLongArray, JObject, JValue};
    use jni::{jni_sig, jni_str};

    let guard = FLUTTER_ASSET_HELPER_CLASS.lock().unwrap();
    let Some(class) = guard.as_ref() else {
        return Err(anyhow!("FlutterAssetHelper jclass not bound yet"));
    };
    let jkey = env
        .new_string(asset_key)
        .map_err(|e| anyhow!("new_string: {e}"))?;
    let args = [JValue::Object(&JObject::from(jkey))];
    let result = env.call_static_method(
        class,
        jni_str!("openAssetFd"),
        jni_sig!("(Ljava/lang/String;)[J"),
        &args,
    )?;
    let arr_obj = result.l().map_err(|e| anyhow!("result: {e}"))?;
    // SAFETY: Java returns `long[]`.
    let long_arr = unsafe { JLongArray::from_raw(env, arr_obj.as_raw() as jni::sys::jarray) };
    let len = long_arr.len(env).map_err(|e| anyhow!("array len: {e}"))?;
    if len < 3 {
        return Err(anyhow!("openAssetFd returned short array"));
    }
    let mut buf = [0i64; 3];
    long_arr
        .get_region(env, 0, &mut buf)
        .map_err(|e| anyhow!("get_region: {e}"))?;
    let fd = buf[0] as i32;
    if fd < 0 {
        crate::diag::logcat_error(&format!(
            "FlutterAssetHelper: asset fd unavailable for {asset_key}"
        ));
        return Err(anyhow!("asset fd unavailable for {asset_key}"));
    }
    Ok((fd, buf[1] as u64, buf[2] as u64))
}

/// 附着当前线程到缓存的 `JavaVM` 并执行 JNI 闭包 / Attaches current thread to cached `JavaVM` and runs JNI closure.
///
/// # 参数 / Parameters
/// - `f` — 接收 `&mut Env` 的闭包 / closure receiving `&mut Env`
///
/// # 返回值 / Returns
/// - 闭包返回值或 `JavaVM not cached` 错误 / closure result or `JavaVM not cached` error
#[cfg(target_os = "android")]
pub fn with_jni_env<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut Env<'_>) -> Result<R>,
{
    let vm_ptr = JAVA_VM.load(Ordering::SeqCst);
    if vm_ptr.is_null() {
        return Err(anyhow!("JavaVM not cached"));
    }
    // SAFETY: pointer came from `JavaVM::get_raw` in a JNI callback.
    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr) };
    vm.attach_current_thread(|env| f(env))
}

/// 将当前线程附着到缓存的 `JavaVM`（无 JNI 操作）/ Attaches current thread to cached `JavaVM` (no JNI work).
///
/// # 返回值 / Returns
/// - `Ok(())` 成功或 `JavaVM` 尚未缓存 / `Ok(())` on success or when `JavaVM` not yet cached
#[cfg(target_os = "android")]
pub fn attach_java_vm() -> Result<()> {
    let vm_ptr = JAVA_VM.load(Ordering::SeqCst);
    if vm_ptr.is_null() {
        return Ok(());
    }
    // SAFETY: pointer came from `JavaVM::get_raw` in a JNI callback.
    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr) };
    vm.attach_current_thread(|_| Ok(()))
}

#[cfg(target_os = "android")]
static PLUGIN_CLASS: Mutex<Option<Global<JClass>>> = Mutex::new(None);

/// 绑定 `XueHuaVideoPlayerPlugin` 全局 jclass / Binds the `XueHuaVideoPlayerPlugin` global jclass.
#[cfg(target_os = "android")]
pub fn bind_xue_hua_video_player_plugin_class(env: &mut Env, class: JClass) {
    match env.new_global_ref(class) {
        Ok(global) => {
            *PLUGIN_CLASS.lock().unwrap() = Some(global);
        }
        Err(e) => {
            crate::diag::logcat_error(&format!("XueHuaVideoPlayerPlugin jclass bind failed: {e}"));
        }
    }
}

/// 将 Flutter `SurfaceProducer` 调整为协商后的视频分辨率并在主线程重新绑定 Surface /
/// Resizes the Flutter `SurfaceProducer` to the negotiated video resolution and
/// rebinds the surface on the main thread (must complete before overlay refresh).
///
/// # 参数 / Parameters
/// - `player_id` — 播放器 ID / player ID
/// - `width` — 目标宽度（`< 2` 时 no-op）/ target width (no-op when `< 2`)
/// - `height` — 目标高度（`< 2` 时 no-op）/ target height (no-op when `< 2`)
///
/// # 返回值 / Returns
/// - `Ok(())` JNI 调用成功 / `Ok(())` on successful JNI call
#[cfg(target_os = "android")]
pub fn notify_texture_content_size(player_id: i64, width: i32, height: i32) -> Result<()> {
    if width < 2 || height < 2 {
        return Ok(());
    }
    with_jni_env(|env| {
        use jni::objects::JValue;
        use jni::{jni_sig, jni_str};

        let guard = PLUGIN_CLASS.lock().unwrap();
        let Some(class) = guard.as_ref() else {
            return Err(anyhow!("XueHuaVideoPlayerPlugin jclass not bound yet"));
        };
        let args = [
            JValue::Long(player_id),
            JValue::Int(width),
            JValue::Int(height),
        ];
        env.call_static_method(
            class,
            jni_str!("setTextureContentSizeSync"),
            jni_sig!("(JII)V"),
            &args,
        )?;
        Ok(())
    })
}

/// 将 Java `android.view.Surface` 转为 VideoOverlay 用的原生窗口句柄 /
/// Converts a Java `android.view.Surface` to a native window handle for VideoOverlay.
///
/// # 参数 / Parameters
/// - `env` — JNI 环境 / JNI environment
/// - `surface` — Java `Surface` 对象 / Java `Surface` object
///
/// # 返回值 / Returns
/// - `Ok(handle)` `ANativeWindow` 指针整型表示 / `ANativeWindow` pointer as integer
///
/// # 错误 / Errors
/// - `ANativeWindow_fromSurface` 返回 null / `ANativeWindow_fromSurface` returned null
#[cfg(target_os = "android")]
pub fn native_window_handle_from_surface(env: &mut Env, surface: JObject) -> Result<usize> {
    // SAFETY: ANativeWindow_fromSurface is the API GStreamer Android tutorials use.
    let window = unsafe { ANativeWindow_fromSurface(env.get_raw(), surface.as_raw()) };
    if window.is_null() {
        return Err(anyhow!("ANativeWindow_fromSurface returned null"));
    }
    Ok(window as usize)
}

/// 释放由 `ANativeWindow_fromSurface` 获得的窗口句柄 / Releases a window handle obtained from `ANativeWindow_fromSurface`.
///
/// # 参数 / Parameters
/// - `handle` — 原生窗口指针；`0` 为 no-op / native window pointer; `0` is no-op
#[cfg(target_os = "android")]
pub fn release_native_window(handle: usize) {
    if handle == 0 {
        return;
    }
    // SAFETY: handle was obtained from ANativeWindow_fromSurface.
    unsafe {
        ANativeWindow_release(handle as *mut std::ffi::c_void);
    }
}
