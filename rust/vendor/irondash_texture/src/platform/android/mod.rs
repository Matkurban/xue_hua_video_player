use std::{
    cell::RefCell,
    collections::HashMap,
    marker::PhantomData,
    slice,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use irondash_engine_context::EngineContext;
use jni::errors::LogErrorAndDefault;
use jni::objects::{Global, JClass, JObject};
use jni::{jni_sig, jni_str, Env, EnvUnowned, NativeMethod};
use ndk_sys::{
    AHardwareBuffer_Format, ANativeWindow, ANativeWindow_Buffer, ANativeWindow_acquire,
    ANativeWindow_fromSurface, ANativeWindow_lock, ANativeWindow_release,
    ANativeWindow_setBuffersGeometry, ANativeWindow_unlockAndPost,
};
use once_cell::sync::Lazy;

use crate::{
    log::OkLog, BoxedPixelData, Error, PayloadProvider, PixelFormat, PlatformTextureWithProvider,
    PlatformTextureWithoutProvider, Result,
};

#[derive(PartialEq, Eq, Clone, Copy)]
struct Geometry {
    width: i32,
    height: i32,
    format: i32,
}

struct SurfaceProducerSlot {
    /// Stored as `usize` because raw pointers are not `Send`.
    native_window: usize,
    producer: Option<Global<JObject<'static>>>,
}

fn slot_window_ptr(slot: &SurfaceProducerSlot) -> *mut ANativeWindow {
    slot.native_window as *mut ANativeWindow
}

fn set_slot_window_ptr(slot: &mut SurfaceProducerSlot, wnd: *mut ANativeWindow) {
    slot.native_window = wnd as usize;
}

static SURFACE_PRODUCER_SLOTS: Lazy<Mutex<HashMap<i64, SurfaceProducerSlot>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static SURFACE_PRODUCER_NATIVES_REGISTERED: AtomicBool = AtomicBool::new(false);

const SURFACE_PRODUCER_CALLBACK_JAVA: &str =
    "com.flutter_rust_bridge.xue_hua_video_player.IrondashSurfaceProducerCallback";

#[cfg(target_os = "android")]
fn android_log_info(msg: &str) {
    use std::ffi::CString;
    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(
            prio: i32,
            tag: *const std::ffi::c_char,
            text: *const std::ffi::c_char,
        ) -> i32;
    }
    let tag = CString::new("xue_hua_video_player").expect("tag");
    let bytes = msg.as_bytes();
    let len = bytes.len().min(3500);
    let text = CString::new(&bytes[..len]).unwrap_or_else(|_| CString::new("…").unwrap());
    unsafe {
        __android_log_write(4, tag.as_ptr(), text.as_ptr());
    }
}

#[cfg(not(target_os = "android"))]
fn android_log_info(_msg: &str) {}

fn load_app_class<'local>(env: &mut Env<'local>, name: &str) -> Result<JClass<'local>> {
    let class_loader = EngineContext::get_class_loader()?;
    let class_name = env.new_string(name)?;
    let obj = env.call_method(
        class_loader.as_obj(),
        jni_str!("loadClass"),
        jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
        &[(&class_name).into()],
    )?;
    if env.exception_check() {
        env.exception_clear();
        return Err(Error::TextureRegistrationFailed);
    }
    env.cast_local::<JClass>(obj.l()?).map_err(Into::into)
}

fn register_surface_producer_slot(id: i64, producer: Global<JObject<'static>>) {
    SURFACE_PRODUCER_SLOTS.lock().unwrap().insert(
        id,
        SurfaceProducerSlot {
            native_window: 0,
            producer: Some(producer),
        },
    );
}

fn unregister_surface_producer_slot(id: i64) {
    SURFACE_PRODUCER_SLOTS.lock().unwrap().remove(&id);
}

fn release_surface_producer_slot_window(id: i64) {
    if let Some(slot) = SURFACE_PRODUCER_SLOTS.lock().unwrap().get_mut(&id) {
        let wnd = slot_window_ptr(slot);
        if !wnd.is_null() {
            unsafe {
                ANativeWindow_release(wnd);
            }
            set_slot_window_ptr(slot, std::ptr::null_mut());
        }
    }
}

fn set_surface_producer_slot_window(id: i64, wnd: *mut ANativeWindow) {
    if let Some(slot) = SURFACE_PRODUCER_SLOTS.lock().unwrap().get_mut(&id) {
        let old = slot_window_ptr(slot);
        if !old.is_null() && old != wnd {
            unsafe {
                ANativeWindow_release(old);
            }
        }
        if old == wnd {
            unsafe {
                ANativeWindow_release(wnd);
            }
        } else {
            set_slot_window_ptr(slot, wnd);
        }
    } else {
        unsafe {
            ANativeWindow_release(wnd);
        }
    }
}

fn refresh_surface_producer_slot_window(env: &mut Env<'_>, texture_id: i64) -> Result<()> {
    let mut slots = SURFACE_PRODUCER_SLOTS.lock().unwrap();
    let slot = slots
        .get_mut(&texture_id)
        .ok_or(Error::TextureRegistrationFailed)?;
    let producer = slot
        .producer
        .as_ref()
        .ok_or(Error::TextureRegistrationFailed)?;
    let surface = env
        .call_method(
            producer.as_obj(),
            jni_str!("getSurface"),
            jni_sig!("()Landroid/view/Surface;"),
            &[],
        )?
        .l()?;
    if env.is_same_object(&surface, JObject::null())? {
        let wnd = slot_window_ptr(slot);
        if !wnd.is_null() {
            unsafe {
                ANativeWindow_release(wnd);
            }
            set_slot_window_ptr(slot, std::ptr::null_mut());
        }
        return Err(Error::TextureRegistrationFailed);
    }
    let new_window = native_window_from_surface(env, &surface)?;
    let old = slot_window_ptr(slot);
    if !old.is_null() && old != new_window {
        unsafe {
            ANativeWindow_release(old);
        }
    }
    if old == new_window {
        unsafe {
            ANativeWindow_release(new_window);
        }
    } else {
        set_slot_window_ptr(slot, new_window);
    }
    Ok(())
}

extern "system" fn native_on_surface_available(
    mut unowned_env: EnvUnowned<'_>,
    _class: jni::objects::JClass<'_>,
    texture_id: jni::sys::jlong,
) {
    log::info!(
        "irondash_texture: surface_producer onSurfaceAvailable id={texture_id}"
    );
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            refresh_surface_producer_slot_window(env, texture_id).ok_log();
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}

extern "system" fn native_on_surface_cleanup(
    _env: EnvUnowned<'_>,
    _class: jni::objects::JClass<'_>,
    texture_id: jni::sys::jlong,
) {
    log::info!(
        "irondash_texture: surface_producer onSurfaceCleanup id={texture_id}"
    );
    release_surface_producer_slot_window(texture_id);
    // PlatformTexture refreshes its cached pointer on the next mark_frame_available.
}

fn ensure_surface_producer_natives_registered(env: &mut Env<'_>) -> Result<()> {
    if SURFACE_PRODUCER_NATIVES_REGISTERED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }

    let class = load_app_class(env, SURFACE_PRODUCER_CALLBACK_JAVA)?;
    let methods = [
        unsafe {
            NativeMethod::from_raw_parts(
                jni_str!("nativeOnSurfaceAvailable"),
                jni_str!("(J)V"),
                native_on_surface_available as *mut _,
            )
        },
        unsafe {
            NativeMethod::from_raw_parts(
                jni_str!("nativeOnSurfaceCleanup"),
                jni_str!("(J)V"),
                native_on_surface_cleanup as *mut _,
            )
        },
    ];
    unsafe {
        env.register_native_methods(class, &methods)?;
    }
    Ok(())
}

pub struct PlatformTexture<Type> {
    id: i64,
    texture_entry: Global<JObject<'static>>,
    surface: RefCell<Global<JObject<'static>>>,
    native_window: RefCell<*mut ANativeWindow>,
    /// When true, `texture_entry` is a `SurfaceProducer` and surfaces must be
    /// obtained via `getSurface()` rather than wrapping a `SurfaceTexture`.
    uses_surface_producer: bool,
    last_geometry: RefCell<Option<Geometry>>,
    pixel_data_provider: Option<Arc<dyn PayloadProvider<BoxedPixelData>>>,
    _phantom: PhantomData<Type>,
}

pub(crate) const PIXEL_DATA_FORMAT: PixelFormat = PixelFormat::RGBA;

fn native_window_from_surface(env: &mut Env<'_>, surface: &JObject) -> Result<*mut ANativeWindow> {
    let native_window = unsafe {
        ANativeWindow_fromSurface(
            env.get_raw().cast(),
            surface.as_raw(),
        )
    };
    if native_window.is_null() {
        Err(Error::TextureRegistrationFailed)
    } else {
        Ok(native_window)
    }
}

impl<Type> PlatformTexture<Type> {
    pub fn id(&self) -> i64 {
        self.id
    }

    fn has_surface_producer_api(env: &mut Env<'_>, texture_registry: &JObject) -> Result<bool> {
        let class = env.get_object_class(texture_registry)?;
        match env.get_method_id(
            &class,
            jni_str!("createSurfaceProducer"),
            jni_sig!("()Lio/flutter/view/TextureRegistry$SurfaceProducer;"),
        ) {
            Ok(_) => Ok(true),
            Err(_) => {
                if env.exception_check() {
                    let _ = env.exception_clear();
                }
                Ok(false)
            }
        }
    }

    fn try_create_surface_producer<'local>(
        env: &mut Env<'local>,
        texture_registry: &JObject,
    ) -> Option<JObject<'local>> {
        match env.call_method(
            texture_registry,
            jni_str!("createSurfaceProducer"),
            jni_sig!("()Lio/flutter/view/TextureRegistry$SurfaceProducer;"),
            &[],
        ) {
            Ok(v) => v.l().ok(),
            Err(e) => {
                log::error!("irondash_texture: createSurfaceProducer JNI call failed: {e}");
                if env.exception_check() {
                    let _ = env.exception_clear();
                }
                None
            }
        }
    }

    fn install_surface_producer_callback(
        env: &mut Env<'_>,
        producer: &JObject,
        texture_id: i64,
    ) -> Result<()> {
        android_log_info("irondash_texture: register_surface_producer natives");
        ensure_surface_producer_natives_registered(env)?;
        let callback_class = load_app_class(env, SURFACE_PRODUCER_CALLBACK_JAVA)?;
        let callback = env.new_object(callback_class, jni_sig!("(J)V"), &[texture_id.into()])?;
        env.call_method(
            producer,
            jni_str!("setCallback"),
            jni_sig!("(Lio/flutter/view/TextureRegistry$SurfaceProducer$Callback;)V"),
            &[(&callback).into()],
        )?;
        Ok(())
    }

    fn new_from_surface_producer(
        env: &mut Env<'_>,
        producer: &JObject,
        pixel_buffer_provider: Option<Arc<dyn PayloadProvider<BoxedPixelData>>>,
    ) -> Result<Self> {
        let id = env
            .call_method(producer, jni_str!("id"), jni_sig!("()J"), &[])?
            .j()?;
        let producer_global = env.new_global_ref(producer)?;
        register_surface_producer_slot(id, env.new_global_ref(producer)?);
        Self::install_surface_producer_callback(env, producer, id)?;

        log::info!("irondash_texture: using surface_producer path (id={id})");

        Ok(Self {
            id,
            texture_entry: producer_global,
            surface: RefCell::new(env.new_global_ref(JObject::null())?),
            native_window: RefCell::new(std::ptr::null_mut()),
            uses_surface_producer: true,
            last_geometry: RefCell::new(None),
            pixel_data_provider: pixel_buffer_provider,
            _phantom: PhantomData {},
        })
    }

    fn new_from_surface_texture(
        env: &mut Env<'_>,
        texture_registry: &JObject,
        pixel_buffer_provider: Option<Arc<dyn PayloadProvider<BoxedPixelData>>>,
    ) -> Result<Self> {
        log::warn!("irondash_texture: using legacy surface_texture path");

        env.with_local_frame(16, |env| {
            let texture_entry = env
                .call_method(
                    texture_registry,
                    jni_str!("createSurfaceTexture"),
                    jni_sig!("()Lio/flutter/view/TextureRegistry$SurfaceTextureEntry;"),
                    &[],
                )?
                .l()?;
            let surface_texture = env
                .call_method(
                    &texture_entry,
                    jni_str!("surfaceTexture"),
                    jni_sig!("()Landroid/graphics/SurfaceTexture;"),
                    &[],
                )?
                .l()?;
            let surface_class = env.find_class(jni_str!("android/view/Surface"))?;

            let surface = env.new_object(
                surface_class,
                jni_sig!("(Landroid/graphics/SurfaceTexture;)V"),
                &[(&surface_texture).into()],
            )?;

            let native_window = native_window_from_surface(env, &surface)?;
            let id = env
                .call_method(&texture_entry, jni_str!("id"), jni_sig!("()J"), &[])?
                .j()?;

            Ok(Self {
                id,
                texture_entry: env.new_global_ref(texture_entry)?,
                surface: RefCell::new(env.new_global_ref(surface)?),
                native_window: RefCell::new(native_window),
                uses_surface_producer: false,
                last_geometry: RefCell::new(None),
                pixel_data_provider: pixel_buffer_provider,
                _phantom: PhantomData {},
            })
        })
    }

    fn new(
        engine_handle: i64,
        pixel_buffer_provider: Option<Arc<dyn PayloadProvider<BoxedPixelData>>>,
    ) -> Result<Self> {
        android_log_info("irondash_texture: PlatformTexture::new enter");
        let java_vm = EngineContext::get_java_vm()?;
        java_vm.attach_current_thread(|env| -> Result<Self> {
            android_log_info("irondash_texture: attached JNI for texture create");
            let engine_context = EngineContext::get()?;
            let texture_registry = engine_context.get_texture_registry(engine_handle)?;
            let registry_obj = texture_registry.as_obj();

            if Self::has_surface_producer_api(env, registry_obj)? {
                android_log_info("irondash_texture: trying surface_producer");
                let producer = Self::try_create_surface_producer(env, registry_obj)
                    .ok_or(Error::TextureRegistrationFailed)?;
                Self::new_from_surface_producer(env, &producer, pixel_buffer_provider)
            } else {
                android_log_info("irondash_texture: falling back to surface_texture");
                Self::new_from_surface_texture(env, registry_obj, pixel_buffer_provider)
            }
        })
    }

    fn refresh_native_window(&self, env: &mut Env<'_>) -> Result<()> {
        if !self.uses_surface_producer {
            return Ok(());
        }
        let surface = env
            .call_method(
                self.texture_entry.as_obj(),
                jni_str!("getSurface"),
                jni_sig!("()Landroid/view/Surface;"),
                &[],
            )?
            .l()?;
        if env.is_same_object(&surface, JObject::null())? {
            let wnd = *self.native_window.borrow();
            if !wnd.is_null() {
                unsafe {
                    ANativeWindow_release(wnd);
                }
                *self.native_window.borrow_mut() = std::ptr::null_mut();
            }
            *self.surface.borrow_mut() = env.new_global_ref(JObject::null())?;
            return Err(Error::TextureRegistrationFailed);
        }

        if env.is_same_object(self.surface.borrow().as_obj(), &surface)? {
            return Ok(());
        }

        let new_window = native_window_from_surface(env, &surface)?;
        set_surface_producer_slot_window(self.id, new_window);
        *self.native_window.borrow_mut() = new_window;
        *self.surface.borrow_mut() = env.new_global_ref(surface)?;
        Ok(())
    }

    fn native_window_ptr(&self) -> *mut ANativeWindow {
        if self.uses_surface_producer {
            SURFACE_PRODUCER_SLOTS
                .lock()
                .unwrap()
                .get(&self.id)
                .map(|slot| slot_window_ptr(slot))
                .unwrap_or(std::ptr::null_mut())
        } else {
            *self.native_window.borrow()
        }
    }

    fn destroy(&mut self) -> Result<()> {
        let java_vm = EngineContext::get_java_vm()?;
        java_vm.attach_current_thread(|env| -> Result<()> {
            env.call_method(
                self.texture_entry.as_obj(),
                jni_str!("release"),
                jni_sig!("()V"),
                &[],
            )?;
            if self.uses_surface_producer {
                let wnd = self.native_window_ptr();
                if !wnd.is_null() {
                    unsafe {
                        ANativeWindow_release(wnd);
                    }
                }
                unregister_surface_producer_slot(self.id);
            } else {
                let wnd = *self.native_window.borrow_mut();
                if !wnd.is_null() {
                    unsafe {
                        ANativeWindow_release(wnd);
                    }
                    *self.native_window.borrow_mut() = std::ptr::null_mut();
                }
            }
            Ok(())
        })
    }

    pub fn mark_frame_available(&self) -> Result<()> {
        if let Some(provider) = self.pixel_data_provider.as_ref() {
            let java_vm = EngineContext::get_java_vm()?;
            java_vm.attach_current_thread(|env| -> Result<()> {
                let payload = provider.get_payload();
                let payload = payload.get();
                let geometry = Geometry {
                    width: payload.width,
                    height: payload.height,
                    format: AHardwareBuffer_Format::AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM.0 as i32,
                };

                if self.uses_surface_producer {
                    env.call_method(
                        self.texture_entry.as_obj(),
                        jni_str!("setSize"),
                        jni_sig!("(II)V"),
                        &[geometry.width.into(), geometry.height.into()],
                    )?;
                    self.refresh_native_window(env)?;
                }

                let native_window = self.native_window_ptr();
                if native_window.is_null() {
                    return Err(Error::TextureRegistrationFailed);
                }

                let mut last_geometry = self.last_geometry.borrow_mut();
                if *last_geometry != Some(geometry) {
                    unsafe {
                        ANativeWindow_setBuffersGeometry(
                            native_window,
                            geometry.width,
                            geometry.height,
                            geometry.format,
                        );
                    }
                    last_geometry.replace(geometry);
                }
                let mut buf: ANativeWindow_Buffer = unsafe { std::mem::zeroed() };

                let lock_result = unsafe {
                    ANativeWindow_lock(native_window, &mut buf as *mut _, std::ptr::null_mut())
                };
                if lock_result != 0 {
                    log::error!(
                        "irondash_texture: ANativeWindow_lock failed: {lock_result} \
                         (id={}, {}x{})",
                        self.id,
                        geometry.width,
                        geometry.height
                    );
                    return Err(Error::TextureRegistrationFailed);
                }

                let expected_payload_len =
                    (payload.width as usize) * (payload.height as usize) * 4;
                if payload.data.len() != expected_payload_len {
                    log::error!(
                        "irondash_texture: payload size mismatch id={} expected={expected_payload_len} \
                         got={} ({}x{})",
                        self.id,
                        payload.data.len(),
                        payload.width,
                        payload.height
                    );
                    unsafe {
                        ANativeWindow_unlockAndPost(native_window);
                    }
                    return Err(Error::TextureRegistrationFailed);
                }

                let data = unsafe {
                    slice::from_raw_parts_mut(
                        buf.bits as *mut u8,
                        (buf.height * buf.stride * 4) as usize,
                    )
                };

                let copy_height = std::cmp::min(payload.height, buf.height);
                if buf.stride == payload.width
                    && buf.stride == buf.width
                    && (buf.stride * buf.height * 4) as usize == payload.data.len()
                {
                    data[..payload.data.len()].copy_from_slice(payload.data);
                } else {
                    let src_stride = payload.width * 4;
                    let dst_stride = buf.stride * 4;
                    let row_bytes = std::cmp::min(src_stride, dst_stride);
                    let mut src_offset: usize = 0;
                    let mut dst_offset: usize = 0;
                    for _ in 0..copy_height {
                        let src_slice =
                            &payload.data[src_offset..src_offset + row_bytes as usize];
                        let dst_slice =
                            &mut data[dst_offset..dst_offset + row_bytes as usize];
                        dst_slice.copy_from_slice(src_slice);
                        src_offset += src_stride as usize;
                        dst_offset += dst_stride as usize;
                    }
                }

                unsafe { ANativeWindow_unlockAndPost(native_window) };

                if self.uses_surface_producer {
                    let _ = env.call_method(
                        self.texture_entry.as_obj(),
                        jni_str!("scheduleFrame"),
                        jni_sig!("()V"),
                        &[],
                    );
                }
                Ok(())
            })?;
        }
        Ok(())
    }
}

impl<Type> Drop for PlatformTexture<Type> {
    fn drop(&mut self) {
        self.destroy().ok_log();
    }
}

impl PlatformTextureWithProvider for BoxedPixelData {
    fn create_texture(
        engine_handle: i64,
        payload_provider: Arc<dyn PayloadProvider<Self>>,
    ) -> Result<PlatformTexture<BoxedPixelData>> {
        PlatformTexture::new(engine_handle, Some(payload_provider))
    }
}

pub struct NativeWindow {
    native_window: *mut ANativeWindow,
}

impl NativeWindow {
    fn new(native_window: *mut ANativeWindow) -> Self {
        unsafe { ANativeWindow_acquire(native_window) };
        Self { native_window }
    }

    pub fn get_native_window(&self) -> *mut ANativeWindow {
        self.native_window
    }
}

impl Clone for NativeWindow {
    fn clone(&self) -> Self {
        Self::new(self.native_window)
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            ANativeWindow_release(self.native_window);
        }
    }
}

impl PlatformTextureWithoutProvider for NativeWindow {
    fn create_texture(engine_handle: i64) -> Result<PlatformTexture<NativeWindow>> {
        PlatformTexture::new(engine_handle, None)
    }

    fn get(texture: &PlatformTexture<Self>) -> Self {
        Self::new(texture.native_window_ptr())
    }
}

pub struct Surface(pub Global<JObject<'static>>);

impl PlatformTextureWithoutProvider for Surface {
    fn create_texture(engine_handle: i64) -> Result<PlatformTexture<Surface>> {
        PlatformTexture::new(engine_handle, None)
    }

    fn get(texture: &PlatformTexture<Self>) -> Self {
        let java_vm = EngineContext::get_java_vm().expect("java vm");
        let surface = java_vm
            .attach_current_thread(|env| -> Result<Global<JObject<'static>>> {
                Ok(env.new_global_ref(texture.surface.borrow().as_obj())?)
            })
            .expect("global ref");
        Self(surface)
    }
}
