mod env;
#[cfg(target_os = "ios")]
mod ios_plugins;
mod tls;

use anyhow::{anyhow, Result};

use crate::gst_runtime::run_on_gst_thread;

/// Ensures `gst::init()` runs exactly once for the process.
pub fn ensure_gst_init() -> Result<()> {
    use std::sync::Once;
    static INIT: Once = Once::new();
    static mut RESULT: Option<Result<()>> = None;
    // SAFETY: guarded by Once, only written inside call_once.
    unsafe {
        INIT.call_once(|| {
            RESULT = Some((|| {
                #[cfg(target_os = "ios")]
                env::setup_ios_env();
                #[cfg(target_os = "macos")]
                env::setup_macos_env();
                crate::gst_runtime::ensure_gst_runtime();
                #[cfg(target_os = "android")]
                {
                    crate::android_gst::ensure_gst_init_android()?;
                }
                #[cfg(not(target_os = "android"))]
                {
                    run_on_gst_thread(|| {
                        #[cfg(target_os = "ios")]
                        {
                            ios_plugins::register_ios_static_plugins();
                            tls::register_gio_tls_backend();
                        }
                        #[cfg(target_os = "macos")]
                        tls::register_gio_tls_backend();
                        Ok(())
                    })?;
                }
                Ok(())
            })());
        });
        match &*std::ptr::addr_of!(RESULT) {
            Some(Ok(())) => Ok(()),
            Some(Err(e)) => Err(anyhow!("{e}")),
            None => Err(anyhow!("gst init state missing")),
        }
    }
}
