use jni::objects::{Global, JClass, JObject};
use jni::{jni_sig, jni_str, Env, JavaVM};

mod notifier;
use notifier::*;

mod jni_context;
mod mini_run_loop;
mod sys;

use crate::{EngineContext, Error, Result};

use self::jni_context::JniContext;

pub(crate) type FlutterView = Global<JObject<'static>>;
pub(crate) type FlutterTextureRegistry = Global<JObject<'static>>;
pub(crate) type FlutterBinaryMessenger = Global<JObject<'static>>;
pub(crate) type Activity = Global<JObject<'static>>;

pub(crate) struct PlatformContext {
    java_vm: &'static JavaVM,
    class_loader: &'static Global<JObject<'static>>,
    destroy_notifier: Option<Notifier>,
}

impl PlatformContext {
    pub fn perform_on_main_thread(f: impl FnOnce() + Send + 'static) -> Result<()> {
        JniContext::get()?.schedule_on_main_thread(f);
        Ok(())
    }

    pub fn is_main_thread() -> Result<bool> {
        Ok(JniContext::get()?.is_main_thread())
    }

    pub fn get_java_vm() -> Result<&'static JavaVM> {
        Ok(JniContext::get()?.java_vm())
    }

    pub fn get_class_loader() -> Result<&'static Global<JObject<'static>>> {
        Ok(JniContext::get()?.class_loader())
    }

    pub fn new() -> Result<Self> {
        let context = JniContext::get()?;
        let mut res = Self {
            java_vm: context.java_vm(),
            class_loader: context.class_loader(),
            destroy_notifier: None,
        };
        res.initialize()?;
        Ok(res)
    }

    fn initialize(&mut self) -> Result<()> {
        let notifier = Notifier::new(move |env, data| {
            let handle = env
                .call_method(data, jni_str!("longValue"), jni_sig!("()J"), &[])
                .ok()
                .and_then(|v| v.j().ok());
            if let (Some(handle), Some(engine_context)) = //
                (handle, EngineContext::try_get())
            {
                engine_context.on_engine_destroyed(handle);
            }
        })?;
        self.java_vm.attach_current_thread(|env| -> Result<()> {
            let class = Self::get_plugin_class(env, &self.class_loader)?;
            env.call_static_method(
                class,
                jni_str!("registerDestroyListener"),
                jni_sig!("(Ldev/irondash/engine_context/Notifier;)V"),
                &[notifier.as_obj().into()],
            )?;
            Ok(())
        })?;
        self.destroy_notifier = Some(notifier);
        Ok(())
    }

    fn get_plugin_class<'a>(
        env: &mut Env<'a>,
        class_loader: &'static Global<JObject<'static>>,
    ) -> Result<JClass<'a>> {
        let class_name =
            env.new_string("dev.irondash.engine_context.IrondashEngineContextPlugin")?;
        let plugin_class = env.call_method(
            class_loader.as_obj(),
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[(&class_name).into()],
        );

        if env.exception_check() {
            env.exception_clear();
            return Err(Error::PluginNotLoaded);
        }

        let plugin_class = plugin_class?.l()?;
        env.cast_local::<JClass>(plugin_class).map_err(Into::into)
    }

    pub fn get_activity(&self, handle: i64) -> Result<Activity> {
        self.java_vm.attach_current_thread(|env| {
            let class = Self::get_plugin_class(env, &self.class_loader)?;
            let activity = env
                .call_static_method(
                    class,
                    jni_str!("getActivity"),
                    jni_sig!("(J)Landroid/app/Activity;"),
                    &[handle.into()],
                )?
                .l()?;
            if env.is_same_object(&activity, JObject::null())? {
                Err(Error::InvalidHandle)
            } else {
                Ok(env.new_global_ref(activity)?)
            }
        })
    }

    pub fn get_flutter_view(&self, handle: i64) -> Result<FlutterView> {
        self.java_vm.attach_current_thread(|env| {
            let class = Self::get_plugin_class(env, &self.class_loader)?;
            let view = env
                .call_static_method(
                    class,
                    jni_str!("getFlutterView"),
                    jni_sig!("(J)Landroid/view/View;"),
                    &[handle.into()],
                )?
                .l()?;
            if env.is_same_object(&view, JObject::null())? {
                Err(Error::InvalidHandle)
            } else {
                Ok(env.new_global_ref(view)?)
            }
        })
    }

    pub fn get_binary_messenger(&self, handle: i64) -> Result<FlutterBinaryMessenger> {
        self.java_vm.attach_current_thread(|env| {
            let class = Self::get_plugin_class(env, &self.class_loader)?;
            let messenger = env
                .call_static_method(
                    class,
                    jni_str!("getBinaryMessenger"),
                    jni_sig!("(J)Lio/flutter/plugin/common/BinaryMessenger;"),
                    &[handle.into()],
                )?
                .l()?;
            if env.is_same_object(&messenger, JObject::null())? {
                Err(Error::InvalidHandle)
            } else {
                Ok(env.new_global_ref(messenger)?)
            }
        })
    }

    pub fn get_texture_registry(&self, handle: i64) -> Result<FlutterTextureRegistry> {
        self.java_vm.attach_current_thread(|env| {
            let class = Self::get_plugin_class(env, &self.class_loader)?;
            let registry = env
                .call_static_method(
                    class,
                    jni_str!("getTextureRegistry"),
                    jni_sig!("(J)Lio/flutter/view/TextureRegistry;"),
                    &[handle.into()],
                )?
                .l()?;
            if env.is_same_object(&registry, JObject::null())? {
                Err(Error::InvalidHandle)
            } else {
                Ok(env.new_global_ref(registry)?)
            }
        })
    }
}
