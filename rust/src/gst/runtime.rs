//! Dedicated `xhvp-gst` GStreamer thread dispatch API.
//!
//! Android uses a poll-loop backend ([`super::android_runtime`]) without GLib
//! `MainLoop`. Other platforms use [`super::runtime_glib`].

#[cfg(target_os = "android")]
pub use super::android_runtime::{
    ensure_gst_runtime, run_on_gst_thread, spawn_on_gst_thread, spawn_on_gst_thread_and_wait,
    BusPollToken, PositionPollToken,
};

#[cfg(not(target_os = "android"))]
pub use super::runtime_glib::{
    ensure_gst_runtime, gst_main_context, run_on_gst_thread, spawn_on_gst_thread,
    spawn_on_gst_thread_and_wait,
};
