use std::{
    ffi::c_void,
    marker::PhantomData,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use core_foundation::{
    base::{CFType, TCFType},
    dictionary::CFDictionaryRef,
    mach_port::CFAllocatorRef,
};
use irondash_engine_context::EngineContext;
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, extern_class, extern_methods, msg_send, AnyThread, DefinedClass};

use crate::{
    log::OkLog,
    platform::platform_impl::io_surface::{
        kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow, kIOSurfaceHeight, kIOSurfacePixelFormat,
        kIOSurfaceWidth,
    },
    BoxedIOSurface, BoxedPixelData, IOSurfaceProvider, PayloadProvider, PixelFormat,
    PlatformTextureWithProvider, Result,
};

use self::io_surface::{IOSurface, IOSurfaceGetHeight, IOSurfaceGetWidth, IOSurfaceRef};
pub(crate) mod io_surface;

pub struct PlatformTexture<Type> {
    id: i64,
    engine_handle: i64,
    _texture_objc: Retained<IrondashTexture>,
    _phantom: PhantomData<Type>,
    update_requested: Arc<AtomicBool>,
}

impl<Type> PlatformTexture<Type> {
    pub fn id(&self) -> i64 {
        self.id
    }
}

extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(PartialEq, Eq, Hash)]
    pub struct FlutterTextureRegistry;
);

impl FlutterTextureRegistry {
    extern_methods!(
        #[allow(non_snake_case)]
        #[unsafe(method(registerTexture:))]
        pub unsafe fn registerTexture(&self, texture: &NSObject) -> i64;

        #[allow(non_snake_case)]
        #[unsafe(method(textureFrameAvailable:))]
        pub unsafe fn textureFrameAvailable(&self, textureId: i64);

        #[allow(non_snake_case)]
        #[unsafe(method(unregisterTexture:))]
        pub unsafe fn unregisterTexture(&self, textureId: i64);
    );
}

pub(crate) const PIXEL_DATA_FORMAT: PixelFormat = PixelFormat::RGBA;

impl<Type> PlatformTexture<Type> {
    fn texture_registery(engine_handle: i64) -> Result<Retained<FlutterTextureRegistry>> {
        Ok(unsafe {
            Retained::cast_unchecked(EngineContext::get()?.get_texture_registry(engine_handle)?)
        })
    }

    pub fn new(
        engine_handle: i64,
        provider: Arc<dyn PayloadProvider<BoxedIOSurface>>,
    ) -> Result<Self> {
        let update_requested = Arc::new(AtomicBool::new(false));
        let provider = Arc::new(SurfaceCache::new(provider, update_requested.clone()));
        let texture_objc = IrondashTexture::new_with_provider(provider);
        let texture_registry = Self::texture_registery(engine_handle)?;
        let id: i64 = unsafe { texture_registry.registerTexture(&texture_objc) };
        Ok(Self {
            id,
            engine_handle,
            _texture_objc: texture_objc,
            _phantom: PhantomData,
            update_requested,
        })
    }

    fn destroy(&mut self) -> Result<()> {
        let texture_registry = Self::texture_registery(self.engine_handle)?;
        unsafe { texture_registry.unregisterTexture(self.id) }
        Ok(())
    }

    pub fn mark_frame_available(&self) -> Result<()> {
        self.update_requested.store(true, Ordering::Release);
        let texture_registry = Self::texture_registery(self.engine_handle)?;
        unsafe { texture_registry.textureFrameAvailable(self.id) };
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
        payload_provider: Arc<dyn PayloadProvider<BoxedPixelData>>,
    ) -> Result<PlatformTexture<BoxedPixelData>> {
        PlatformTexture::<BoxedPixelData>::new(
            engine_handle,
            Arc::new(SurfaceAdapter::new(payload_provider)),
        )
    }
}

impl PlatformTextureWithProvider for BoxedIOSurface {
    fn create_texture(
        engine_handle: i64,
        payload_provider: Arc<dyn PayloadProvider<BoxedIOSurface>>,
    ) -> Result<PlatformTexture<BoxedIOSurface>> {
        PlatformTexture::<BoxedIOSurface>::new(engine_handle, payload_provider)
    }
}

struct IOSurfaceHolder {
    surface: IOSurface,
}

impl IOSurfaceProvider for IOSurfaceHolder {
    fn get(&self) -> &IOSurface {
        &self.surface
    }
}

/// SurfaceCache has two purposes:
/// 1. It makes sure we keep onto the payload while surface is in use.
/// 2. On iOS, which has a bug that requests the texture during every frame
/// regardless of mark_frame_available this reuses existing surface until next
/// call to mark_frame_available.
struct SurfaceCache {
    surface: Mutex<Option<IOSurface>>,
    parent_provider: Arc<dyn PayloadProvider<BoxedIOSurface>>,
    update_requested: Arc<AtomicBool>,
}

impl SurfaceCache {
    fn new(
        parent_provider: Arc<dyn PayloadProvider<BoxedIOSurface>>,
        update_requested: Arc<AtomicBool>,
    ) -> Self {
        Self {
            surface: Mutex::new(None),
            parent_provider,
            update_requested,
        }
    }
}

impl PayloadProvider<BoxedIOSurface> for SurfaceCache {
    fn get_payload(&self) -> BoxedIOSurface {
        let mut surface = self.surface.lock().unwrap();
        if self.update_requested.load(Ordering::Acquire) {
            surface.take();
            self.update_requested.store(false, Ordering::Release);
        }
        let surface =
            surface.get_or_insert_with(|| self.parent_provider.get_payload().get().clone());
        Box::new(IOSurfaceHolder {
            surface: surface.clone(),
        })
    }
}

struct SurfaceAdapter {
    pixel_provider: Arc<dyn PayloadProvider<BoxedPixelData>>,
    cached_surface: Mutex<Option<IOSurface>>,
}

impl SurfaceAdapter {
    fn new(pixel_provider: Arc<dyn PayloadProvider<BoxedPixelData>>) -> Self {
        Self {
            pixel_provider,
            cached_surface: Mutex::new(None),
        }
    }

    fn surface_for_pixel_buffer(&self, width: i32, height: i32) -> IOSurface {
        let mut cached_surface = self.cached_surface.lock().unwrap();
        if let Some(cached_surface) = cached_surface.as_ref() {
            unsafe {
                let surface = cached_surface.as_concrete_TypeRef();
                if IOSurfaceGetWidth(surface) == width as usize
                    && IOSurfaceGetHeight(surface) == height as usize
                {
                    return cached_surface.clone();
                }
            }
        }
        let surface = init_surface(width, height);
        cached_surface.replace(surface.clone());
        surface
    }
}

impl PayloadProvider<BoxedIOSurface> for SurfaceAdapter {
    fn get_payload(&self) -> BoxedIOSurface {
        let buffer = self.pixel_provider.get_payload();
        let buffer = buffer.get();
        let surface = self.surface_for_pixel_buffer(buffer.width, buffer.height);
        surface.upload(buffer.data, (buffer.width * 4) as usize);
        Box::new(IOSurfaceHolder { surface })
    }
}

type CVPixelBufferRef = *mut c_void;

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    fn CVPixelBufferCreateWithIOSurface(
        allocator: CFAllocatorRef,
        surface: IOSurfaceRef,
        pixelBufferAttributes: CFDictionaryRef,
        pixelBufferOut: *mut CVPixelBufferRef,
    ) -> i32;
}

fn do_copy_pixel_buffer(provider: &Arc<dyn PayloadProvider<BoxedIOSurface>>) -> CVPixelBufferRef {
    let surface = provider.get_payload();
    let surface = surface.get();
    let mut buffer: CVPixelBufferRef = std::ptr::null_mut();
    unsafe {
        CVPixelBufferCreateWithIOSurface(
            std::ptr::null_mut(),
            surface.as_CFTypeRef() as *const _,
            std::ptr::null_mut(),
            &mut buffer as *mut _,
        );
    }
    buffer
}

#[allow(clippy::identity_op)]
const fn as_u32_be(array: &[u8; 4]) -> u32 {
    ((array[0] as u32) << 24)
        + ((array[1] as u32) << 16)
        + ((array[2] as u32) << 8)
        + ((array[3] as u32) << 0)
}

fn init_surface(width: i32, height: i32) -> IOSurface {
    use core_foundation::{dictionary::CFDictionary, number::CFNumber, string::CFString};

    let k_width: CFString = unsafe { TCFType::wrap_under_get_rule(kIOSurfaceWidth) };
    let v_width: CFNumber = width.into();

    let k_height: CFString = unsafe { TCFType::wrap_under_get_rule(kIOSurfaceHeight) };
    let v_height: CFNumber = height.into();

    let k_bytes_per_row: CFString = unsafe { TCFType::wrap_under_get_rule(kIOSurfaceBytesPerRow) };
    // Align the row stride to 256 bytes so the surface can back a Metal texture
    // (required by the current Flutter macOS/iOS renderer).
    let aligned_bytes_per_row = (((width as usize) * 4 + 255) / 256) * 256;
    let v_bytes_per_row: CFNumber = (aligned_bytes_per_row as i32).into();

    let k_pixel_format: CFString = unsafe { TCFType::wrap_under_get_rule(kIOSurfacePixelFormat) };
    let v_pixel_format: CFNumber = (as_u32_be(b"BGRA") as i32).into();

    let k_bytes_per_elem: CFString =
        unsafe { TCFType::wrap_under_get_rule(kIOSurfaceBytesPerElement) };
    let v_bytes_per_elem: CFNumber = 4.into();

    let pairs: Vec<(CFString, CFType)> = vec![
        (k_width, v_width.as_CFType()),
        (k_height, v_height.as_CFType()),
        (k_bytes_per_row, v_bytes_per_row.as_CFType()),
        (k_bytes_per_elem, v_bytes_per_elem.as_CFType()),
        (k_pixel_format, v_pixel_format.as_CFType()),
    ];

    IOSurface::new(&CFDictionary::from_CFType_pairs(pairs.as_slice()))
}

struct Ivars {
    payload_provider: Arc<dyn PayloadProvider<BoxedIOSurface>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "IrondashTexture"]
    #[ivars = Ivars]
    struct IrondashTexture;

    impl IrondashTexture {
        #[unsafe(method(copyPixelBuffer))]
        fn copy_pixel_buffer(&self) -> CVPixelBufferRef {
            do_copy_pixel_buffer(&self.ivars().payload_provider)
        }
    }
);

impl IrondashTexture {
    pub fn new_with_provider(
        payload_provider: Arc<dyn PayloadProvider<BoxedIOSurface>>,
    ) -> Retained<Self> {
        let this = Self::alloc().set_ivars(Ivars {
            payload_provider: payload_provider.clone(),
        });
        unsafe { msg_send![super(this), init] }
    }
}
