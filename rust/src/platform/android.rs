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

/// Caches the process `JavaVM` for attaching on the Gst thread.
#[cfg(target_os = "android")]
pub fn store_java_vm(vm: *mut jni::sys::JavaVM) {
    JAVA_VM.store(vm, Ordering::SeqCst);
}

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

/// Invokes `FlutterAssetHelper.openAssetFd` using the cached application jclass.
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

/// Converts a Java `android.view.Surface` to a native window handle for VideoOverlay.
#[cfg(target_os = "android")]
pub fn native_window_handle_from_surface(env: &mut Env, surface: JObject) -> Result<usize> {
    // SAFETY: ANativeWindow_fromSurface is the API GStreamer Android tutorials use.
    let window = unsafe { ANativeWindow_fromSurface(env.get_raw(), surface.as_raw()) };
    if window.is_null() {
        return Err(anyhow!("ANativeWindow_fromSurface returned null"));
    }
    Ok(window as usize)
}

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
