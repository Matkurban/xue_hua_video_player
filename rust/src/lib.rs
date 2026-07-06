pub mod api;
mod android_gst;
#[cfg(target_os = "android")]
mod android_gst_runtime;
mod frb_generated;
mod platform_overlay;
#[cfg(target_os = "macos")]
mod macos_gio_tls;
#[cfg(target_os = "android")]
mod platform_view_android;
mod platform_view_jni;
mod player;

pub(crate) mod diag;
