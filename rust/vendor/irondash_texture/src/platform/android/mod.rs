use std::{cell::RefCell, marker::PhantomData, slice, sync::Arc};

use irondash_engine_context::EngineContext;
use jni::objects::{GlobalRef, JObject};
use ndk_sys::{
    AHardwareBuffer_Format, ANativeWindow, ANativeWindow_Buffer, ANativeWindow_acquire,
    ANativeWindow_fromSurface, ANativeWindow_lock, ANativeWindow_release,
    ANativeWindow_setBuffersGeometry, ANativeWindow_unlockAndPost,
};

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

pub struct PlatformTexture<Type> {
    id: i64,
    texture_entry: GlobalRef,
    surface: GlobalRef,
    native_window: RefCell<*mut ANativeWindow>,
    /// When true, `texture_entry` is a `SurfaceProducer` and surfaces must be
    /// obtained via `getSurface()` rather than wrapping a `SurfaceTexture`.
    uses_surface_producer: bool,
    last_geometry: RefCell<Option<Geometry>>,
    pixel_data_provider: Option<Arc<dyn PayloadProvider<BoxedPixelData>>>,
    _phantom: PhantomData<Type>,
}

pub(crate) const PIXEL_DATA_FORMAT: PixelFormat = PixelFormat::RGBA;

fn native_window_from_surface(
    env: &mut jni::JNIEnv<'_>,
    surface: &JObject,
) -> Result<*mut ANativeWindow> {
    let native_window =
        unsafe { ANativeWindow_fromSurface(env.get_native_interface(), surface.as_raw()) };
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

    fn try_create_surface_producer<'local>(
        env: &mut jni::JNIEnv<'local>,
        texture_registry: &JObject,
    ) -> Option<jni::objects::JObject<'local>> {
        match env.call_method(
            texture_registry,
            "createSurfaceProducer",
            "()Lio/flutter/view/TextureRegistry$SurfaceProducer;",
            &[],
        ) {
            Ok(v) => v.l().ok(),
            Err(_) => {
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_clear();
                }
                None
            }
        }
    }

    fn new_from_surface_producer(
        env: &mut jni::JNIEnv<'_>,
        producer: &JObject,
        pixel_buffer_provider: Option<Arc<dyn PayloadProvider<BoxedPixelData>>>,
    ) -> Result<Self> {
        let surface = env
            .call_method(
                producer,
                "getSurface",
                "()Landroid/view/Surface;",
                &[],
            )?
            .l()?;
        if env.is_same_object(&surface, JObject::null())? {
            return Err(Error::TextureRegistrationFailed);
        }

        let native_window = native_window_from_surface(env, &surface)?;
        let id = env.call_method(producer, "id", "()J", &[])?.j()?;

        Ok(Self {
            id,
            texture_entry: env.new_global_ref(producer)?,
            surface: env.new_global_ref(surface)?,
            native_window: RefCell::new(native_window),
            uses_surface_producer: true,
            last_geometry: RefCell::new(None),
            pixel_data_provider: pixel_buffer_provider,
            _phantom: PhantomData {},
        })
    }

    fn new_from_surface_texture(
        env: &mut jni::JNIEnv<'_>,
        texture_registry: &JObject,
        pixel_buffer_provider: Option<Arc<dyn PayloadProvider<BoxedPixelData>>>,
    ) -> Result<Self> {
        let texture_entry = env
            .call_method(
                texture_registry,
                "createSurfaceTexture",
                "()Lio/flutter/view/TextureRegistry$SurfaceTextureEntry;",
                &[],
            )?
            .l()?;
        let surface_texture = env
            .call_method(
                &texture_entry,
                "surfaceTexture",
                "()Landroid/graphics/SurfaceTexture;",
                &[],
            )?
            .l()?;
        let surface_class = env.find_class("android/view/Surface")?;

        env.push_local_frame(16)?;

        let surface = env.new_object(
            surface_class,
            "(Landroid/graphics/SurfaceTexture;)V",
            &[(&surface_texture).into()],
        )?;

        let native_window = native_window_from_surface(env, &surface)?;
        let id = env.call_method(&texture_entry, "id", "()J", &[])?.j()?;

        let res = Self {
            id,
            texture_entry: env.new_global_ref(texture_entry)?,
            surface: env.new_global_ref(surface)?,
            native_window: RefCell::new(native_window),
            uses_surface_producer: false,
            last_geometry: RefCell::new(None),
            pixel_data_provider: pixel_buffer_provider,
            _phantom: PhantomData {},
        };
        unsafe {
            env.pop_local_frame(&JObject::null())?;
        }
        Ok(res)
    }

    fn new(
        engine_handle: i64,
        pixel_buffer_provider: Option<Arc<dyn PayloadProvider<BoxedPixelData>>>,
    ) -> Result<Self> {
        let java_vm = EngineContext::get_java_vm()?;
        let mut env = java_vm.attach_current_thread()?;
        let engine_context = EngineContext::get()?;
        let texture_registry = engine_context.get_texture_registry(engine_handle)?;

        if let Some(producer) =
            Self::try_create_surface_producer(&mut env, texture_registry.as_obj())
        {
            Self::new_from_surface_producer(&mut env, &producer, pixel_buffer_provider)
        } else {
            Self::new_from_surface_texture(
                &mut env,
                texture_registry.as_obj(),
                pixel_buffer_provider,
            )
        }
    }

    fn refresh_native_window(&self, env: &mut jni::JNIEnv<'_>) -> Result<()> {
        if !self.uses_surface_producer {
            return Ok(());
        }
        let surface = env
            .call_method(
                self.texture_entry.as_obj(),
                "getSurface",
                "()Landroid/view/Surface;",
                &[],
            )?
            .l()?;
        if env.is_same_object(&surface, JObject::null())? {
            return Err(Error::TextureRegistrationFailed);
        }
        let new_window = native_window_from_surface(env, &surface)?;
        let mut slot = self.native_window.borrow_mut();
        unsafe {
            if !slot.is_null() {
                if *slot == new_window {
                    return Ok(());
                }
                ANativeWindow_release(*slot);
            }
            *slot = new_window;
        }
        Ok(())
    }

    fn native_window_ptr(&self) -> *mut ANativeWindow {
        *self.native_window.borrow()
    }

    fn destroy(&mut self) -> Result<()> {
        let java_vm = EngineContext::get_java_vm()?;
        let mut env = java_vm.attach_current_thread()?;
        env.call_method(self.texture_entry.as_obj(), "release", "()V", &[])?;
        let wnd = *self.native_window.borrow_mut();
        if !wnd.is_null() {
            unsafe {
                ANativeWindow_release(wnd);
            }
            *self.native_window.borrow_mut() = std::ptr::null_mut();
        }
        Ok(())
    }

    pub fn mark_frame_available(&self) -> Result<()> {
        if let Some(provider) = self.pixel_data_provider.as_ref() {
            let java_vm = EngineContext::get_java_vm()?;
            let mut env = java_vm.attach_current_thread()?;

            let payload = provider.get_payload();
            let payload = payload.get();
            let geometry = Geometry {
                width: payload.width,
                height: payload.height,
                format: AHardwareBuffer_Format::AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM.0 as i32,
            };

            if self.uses_surface_producer {
                self.refresh_native_window(&mut env)?;
                env.call_method(
                    self.texture_entry.as_obj(),
                    "setSize",
                    "(II)V",
                    &[geometry.width.into(), geometry.height.into()],
                )?;
                self.refresh_native_window(&mut env)?;
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

            let data = unsafe {
                ANativeWindow_lock(native_window, &mut buf as *mut _, std::ptr::null_mut());
                slice::from_raw_parts_mut(
                    buf.bits as *mut u8,
                    (buf.height * buf.stride * 4) as usize,
                )
            };

            if buf.stride == buf.width {
                assert!(buf.stride * buf.height * 4 == payload.data.len() as i32);
                data.copy_from_slice(payload.data);
            } else {
                let src_stride = payload.width * 4;
                let dst_stride = buf.stride * 4;
                let min_stride = std::cmp::min(src_stride, dst_stride);
                let mut src_offset: usize = 0;
                let mut dst_offset: usize = 0;
                for _ in 0..payload.height {
                    let src_slice = &payload.data[src_offset..src_offset + min_stride as usize];
                    let dst_slice = &mut data[dst_offset..dst_offset + min_stride as usize];
                    dst_slice.copy_from_slice(src_slice);
                    src_offset += src_stride as usize;
                    dst_offset += dst_stride as usize;
                }
            }

            unsafe { ANativeWindow_unlockAndPost(native_window) };
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

pub struct Surface(pub GlobalRef);

impl PlatformTextureWithoutProvider for Surface {
    fn create_texture(engine_handle: i64) -> Result<PlatformTexture<Surface>> {
        PlatformTexture::new(engine_handle, None)
    }

    fn get(texture: &PlatformTexture<Self>) -> Self {
        Self(texture.surface.clone())
    }
}
