use std::ffi::c_void;

use core_foundation::runloop::{CFRunLoopGetCurrent, CFRunLoopGetMain};
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{extern_class, extern_methods};

use crate::{Error, Result};

use self::sys::{dispatch_async_f, dispatch_get_main_queue};

mod sys;

pub(crate) struct PlatformContext {}

pub(crate) type FlutterView = Retained<NSObject>;
pub(crate) type FlutterTextureRegistry = Retained<NSObject>;
pub(crate) type FlutterBinaryMessenger = Retained<NSObject>;

extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(PartialEq, Eq, Hash)]
    struct IrondashEngineContextPlugin;
);

impl IrondashEngineContextPlugin {
    extern_methods!(
        #[allow(non_snake_case)]
        #[unsafe(method(getFlutterView:))]
        pub unsafe fn getFlutterView(handle: i64) -> Option<FlutterView>;

        #[allow(non_snake_case)]
        #[unsafe(method(getTextureRegistry:))]
        pub unsafe fn getTextureRegistry(handle: i64) -> Option<FlutterTextureRegistry>;

        #[allow(non_snake_case)]
        #[unsafe(method(getBinaryMessenger:))]
        pub unsafe fn getBinaryMessenger(handle: i64) -> Option<FlutterBinaryMessenger>;

        #[allow(non_snake_case)]
        #[unsafe(method(registerEngineDestroyedCallback:))]
        pub unsafe fn registerEngineDestroyedCallback(callback: extern "C" fn(i64));
    );
}

impl PlatformContext {
    pub fn perform_on_main_thread(f: impl FnOnce() + Send + 'static) -> Result<()> {
        // This could be done through custom run loop source but it
        // is probably not worth the effort. Just use dispatch queue
        // for now.
        let callback: Box<dyn FnOnce()> = Box::new(f);
        let callback = Box::new(callback);
        let callback = Box::into_raw(callback);
        unsafe {
            dispatch_async_f(
                dispatch_get_main_queue(),
                callback as *mut c_void,
                Self::dispatch_work,
            );
        }
        Ok(())
    }

    extern "C" fn dispatch_work(data: *mut c_void) {
        let callback = data as *mut Box<dyn FnOnce()>;
        let callback = unsafe { Box::from_raw(callback) };
        callback();
    }

    pub fn is_main_thread() -> Result<bool> {
        Ok(unsafe { CFRunLoopGetCurrent() == CFRunLoopGetMain() })
    }

    pub fn new() -> Result<Self> {
        let res = Self {};
        res.initialize()?;
        Ok(res)
    }

    fn initialize(&self) -> Result<()> {
        unsafe {
            IrondashEngineContextPlugin::registerEngineDestroyedCallback(on_engine_destroyed);
        }
        Ok(())
    }

    pub fn get_flutter_view(&self, handle: i64) -> Result<FlutterView> {
        unsafe {
            let view = IrondashEngineContextPlugin::getFlutterView(handle);
            match view {
                Some(view) => Ok(view),
                None => Err(Error::InvalidHandle),
            }
        }
    }

    pub fn get_texture_registry(&self, handle: i64) -> Result<FlutterTextureRegistry> {
        unsafe {
            let registry = IrondashEngineContextPlugin::getTextureRegistry(handle);
            match registry {
                Some(registry) => Ok(registry),
                None => Err(Error::InvalidHandle),
            }
        }
    }

    pub fn get_binary_messenger(&self, handle: i64) -> Result<FlutterBinaryMessenger> {
        unsafe {
            let messenger = IrondashEngineContextPlugin::getBinaryMessenger(handle);
            match messenger {
                Some(messenger) => Ok(messenger),
                None => Err(Error::InvalidHandle),
            }
        }
    }
}

extern "C" fn on_engine_destroyed(handle: i64) {
    if let Some(engine_context) = crate::EngineContext::try_get() {
        engine_context.on_engine_destroyed(handle);
    }
}
