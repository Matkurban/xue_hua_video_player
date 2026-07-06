#[cfg(target_os = "android")]
use std::sync::atomic::{AtomicPtr, Ordering};

#[cfg(target_os = "android")]
use anyhow::{anyhow, Result};
#[cfg(target_os = "android")]
use jni::objects::JObject;
#[cfg(target_os = "android")]
use jni::Env;

#[cfg(target_os = "android")]
static JAVA_VM: AtomicPtr<jni::sys::JavaVM> = AtomicPtr::new(std::ptr::null_mut());

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
