//! Android JNI 与 `ANativeWindow` 辅助函数 / Android JNI and `ANativeWindow` helpers.
//!
//! 缓存 `JavaVM`、绑定 Flutter 插件 jclass、从 `Surface` 获取原生窗口句柄，
//! 以及通过 JNI 回调调整纹理内容尺寸。
//!
//! Caches `JavaVM`, binds Flutter plugin jclasses, obtains native window handles from `Surface`,
//! and resizes texture content via JNI callbacks.

#[cfg(target_os = "android")]
use std::sync::{
    atomic::{AtomicBool, AtomicPtr, Ordering},
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
static ANDROID_CONTEXT: Mutex<Option<Global<JObject>>> = Mutex::new(None);

#[cfg(target_os = "android")]
static FLUTTER_ASSET_HELPER_CLASS: Mutex<Option<Global<JClass>>> = Mutex::new(None);

/// Set after `xhvp-gst` permanently attaches via raw JNI (avoids jni-rs TLS keys).
#[cfg(target_os = "android")]
static GST_JVM_ATTACHED: AtomicBool = AtomicBool::new(false);

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

/// Initializes `ndk-context` with the application `Context` (called from Java at startup).
#[cfg(target_os = "android")]
pub fn init_android_context(env: &mut Env, context: JObject) -> Result<()> {
    let global = env
        .new_global_ref(context)
        .map_err(|e| anyhow!("new_global_ref(context): {e}"))?;
    let vm_ptr = env
        .get_java_vm()
        .map_err(|e| anyhow!("get_java_vm: {e}"))?
        .get_raw();
    store_java_vm(vm_ptr);
    // SAFETY: pointers are valid JNIEnv/JavaVM/Context from a JNI callback.
    unsafe {
        ndk_context::initialize_android_context(
            vm_ptr as *mut std::ffi::c_void,
            global.as_obj().as_raw() as *mut std::ffi::c_void,
        );
    }
    *ANDROID_CONTEXT.lock().unwrap() = Some(global);
    crate::diag::logcat_info("ndk-context: Android context initialized");
    Ok(())
}

fn resolve_java_vm_ptr() -> Result<*mut jni::sys::JavaVM> {
    let vm_ptr = ndk_context::android_context().vm() as *mut jni::sys::JavaVM;
    if vm_ptr.is_null() {
        return Err(anyhow!("ndk-context JavaVM is null"));
    }
    store_java_vm(vm_ptr);
    Ok(vm_ptr)
}

fn raw_attach_current_thread(vm_ptr: *mut jni::sys::JavaVM) -> Result<()> {
    if is_thread_jni_attached(vm_ptr) {
        return Ok(());
    }
    // SAFETY: `vm_ptr` came from `store_java_vm` / ndk-context / JNI callbacks.
    let status = unsafe {
        let vm: jni::sys::JavaVM = *vm_ptr;
        let mut env: *mut jni::sys::JNIEnv = std::ptr::null_mut();
        ((*vm).v1_4.AttachCurrentThreadAsDaemon)(
            vm_ptr,
            &mut env as *mut *mut jni::sys::JNIEnv as *mut *mut std::ffi::c_void,
            std::ptr::null_mut(),
        )
    };
    if status != jni::sys::JNI_OK {
        return Err(anyhow!("AttachCurrentThreadAsDaemon failed: {status}"));
    }
    Ok(())
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

/// Returns whether the current native thread is already attached to the cached `JavaVM`.
#[cfg(target_os = "android")]
fn is_thread_jni_attached(vm_ptr: *mut jni::sys::JavaVM) -> bool {
    // SAFETY: `vm_ptr` came from `store_java_vm` / JNI callbacks.
    unsafe {
        let vm: jni::sys::JavaVM = *vm_ptr;
        let mut env: *mut jni::sys::JNIEnv = std::ptr::null_mut();
        ((*vm).v1_4.GetEnv)(
            vm_ptr,
            &mut env as *mut *mut jni::sys::JNIEnv as *mut *mut std::ffi::c_void,
            jni::sys::JNI_VERSION_1_6,
        ) == jni::sys::JNI_OK
    }
}

/// Runs `f` on the current thread using raw `GetEnv` (no jni-rs TLS attach guard).
#[cfg(target_os = "android")]
fn with_attached_env_raw<F, R>(vm_ptr: *mut jni::sys::JavaVM, f: F) -> Result<R>
where
    F: FnOnce(&mut Env<'_>) -> Result<R>,
{
    use jni::{AttachGuard, DEFAULT_LOCAL_FRAME_CAPACITY};

    // SAFETY: caller verified the thread is attached; `GetEnv` returns a valid env pointer.
    unsafe {
        let vm: jni::sys::JavaVM = *vm_ptr;
        let mut env_ptr: *mut jni::sys::JNIEnv = std::ptr::null_mut();
        let status = ((*vm).v1_4.GetEnv)(
            vm_ptr,
            &mut env_ptr as *mut *mut jni::sys::JNIEnv as *mut *mut std::ffi::c_void,
            jni::sys::JNI_VERSION_1_6,
        );
        if status != jni::sys::JNI_OK {
            return Err(anyhow!("GetEnv failed: {status}"));
        }
        let mut guard = AttachGuard::from_unowned(env_ptr);
        guard
            .borrow_env_mut()
            .with_local_frame(DEFAULT_LOCAL_FRAME_CAPACITY, f)
            .map_err(|e| anyhow!("{e}"))
    }
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
    let vm_ptr = resolve_java_vm_ptr().or_else(|_| {
        let ptr = JAVA_VM.load(Ordering::SeqCst);
        if ptr.is_null() {
            Err(anyhow!("JavaVM not cached"))
        } else {
            Ok(ptr)
        }
    })?;
    raw_attach_current_thread(vm_ptr)?;
    with_attached_env_raw(vm_ptr, f)
}

/// 将当前线程附着到缓存的 `JavaVM`（无 JNI 操作）/ Attaches current thread to cached `JavaVM` (no JNI work).
///
/// Uses raw `AttachCurrentThreadAsDaemon` so jni-rs does not allocate a `TLS_ATTACH_GUARD`
/// pthread key (which can exhaust Bionic's TLS key budget on SDK-heavy apps).
///
/// # 返回值 / Returns
/// - `Ok(())` 成功或 `JavaVM` 尚未缓存 / `Ok(())` on success or when `JavaVM` not yet cached
#[cfg(target_os = "android")]
pub fn attach_java_vm() -> Result<()> {
    if GST_JVM_ATTACHED.load(Ordering::SeqCst) {
        return Ok(());
    }
    let vm_ptr = match resolve_java_vm_ptr() {
        Ok(ptr) => ptr,
        Err(_) => {
            let ptr = JAVA_VM.load(Ordering::SeqCst);
            if ptr.is_null() {
                return Ok(());
            }
            ptr
        }
    };
    if is_thread_jni_attached(vm_ptr) {
        GST_JVM_ATTACHED.store(true, Ordering::SeqCst);
        return Ok(());
    }
    raw_attach_current_thread(vm_ptr)?;
    GST_JVM_ATTACHED.store(true, Ordering::SeqCst);
    crate::diag::logcat_info("xhvp-gst: JavaVM attached via raw JNI (no jni-rs TLS)");
    Ok(())
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
