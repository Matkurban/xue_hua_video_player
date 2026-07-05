use std::mem::ManuallyDrop;

use jni::errors::LogErrorAndDefault;
use jni::objects::{Global, JClass, JObject};
use jni::{jni_sig, jni_str, Env, EnvUnowned};

use super::jni_context::JniContext;
use crate::Result;

pub(crate) struct Notifier {
    notifier: Global<JObject<'static>>,
}

type NotifierCallback = dyn Fn(&mut Env, &JObject);

impl Notifier {
    pub fn new<F>(callback: F) -> Result<Self>
    where
        F: Fn(&mut Env, &JObject) + 'static,
    {
        let callback: Box<NotifierCallback> = Box::new(callback);
        let callback = Box::new(callback);

        let context = JniContext::get()?;
        let instance = context.java_vm().attach_current_thread(|env| {
            let class_loader = context.class_loader();
            let class_name = env.new_string("dev.irondash.engine_context.NativeNotifier")?;
            let obj = env
                .call_method(
                    class_loader.as_obj(),
                    jni_str!("loadClass"),
                    jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
                    &[(&class_name).into()],
                )?
                .l()?;
            let notifier_class = env.cast_local::<JClass>(obj)?;
            let callback_addr = Box::into_raw(callback) as i64;
            let instance =
                env.new_object(notifier_class, jni_sig!("(J)V"), &[callback_addr.into()])?;
            env.new_global_ref(instance)
        })?;
        Ok(Self { notifier: instance })
    }

    fn get_native_data(env: &mut Env, obj: &JObject) -> Result<i64> {
        Ok(env
            .get_field(obj, jni_str!("mNativeData"), jni_sig!("J"))?
            .j()?)
    }

    fn set_native_data(env: &mut Env, obj: &JObject, data: i64) -> Result<()> {
        env.set_field(
            obj,
            jni_str!("mNativeData"),
            jni_sig!("J"),
            data.into(),
        )?;
        Ok(())
    }

    pub fn as_obj(&self) -> &JObject<'_> {
        self.notifier.as_obj()
    }
}

impl Drop for Notifier {
    fn drop(&mut self) {
        if let Ok(context) = JniContext::get() {
            let _ = context
                .java_vm()
                .attach_current_thread(|env| -> crate::Result<()> {
                env.call_method(
                    self.notifier.as_obj(),
                    jni_str!("destroy"),
                    jni_sig!("()V"),
                    &[],
                )
                .ok();
                Ok(())
            });
        }
    }
}

#[no_mangle]
extern "system" fn Java_dev_irondash_engine_1context_NativeNotifier_onNotify(
    mut unowned_env: EnvUnowned<'_>,
    obj: JObject<'_>,
    argument: JObject<'_>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let data = Notifier::get_native_data(env, &obj).unwrap_or(0);
            if data != 0 {
                let notify: Box<Box<NotifierCallback>> = unsafe { Box::from_raw(data as *mut _) };
                let notify = ManuallyDrop::new(notify);
                notify(env, &argument);
            }
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}

#[no_mangle]
extern "system" fn Java_dev_irondash_engine_1context_NativeNotifier_destroy(
    mut unowned_env: EnvUnowned<'_>,
    obj: JObject<'_>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let data = Notifier::get_native_data(env, &obj).unwrap_or(0);
            if data != 0 {
                let _notify: Box<Box<NotifierCallback>> = unsafe { Box::from_raw(data as *mut _) };
                Notifier::set_native_data(env, &obj, 0).ok();
            }
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}
