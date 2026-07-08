//! appsink-based video frame source for Flutter external-texture rendering.
//!
//! On Apple / Windows / Linux the pipeline terminates in an `appsink` producing
//! `BGRA` frames. Each decoded frame is copied into a latest-frame double buffer
//! and the registered native texture layer is notified; the native side then
//! pulls the pixels (via [`FrameSink::copy_latest`]) and updates its Flutter
//! external texture. Android keeps `glimagesink` rendering into a texture-backed
//! `Surface`, so it does not use this module.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::{AppSink, AppSinkCallbacks};
use gstreamer_video::{self as gst_video, prelude::VideoFrameExt};
use once_cell::sync::Lazy;
use parking_lot::Mutex;

/// Pixel format delivered to the native texture layer (matches appsink caps).
pub const FRAME_FORMAT_BGRA: gst_video::VideoFormat = gst_video::VideoFormat::Bgra;

/// A decoded BGRA frame. `stride` is the row stride in bytes (may exceed
/// `width * 4` due to GStreamer row padding), so the native side must honor it.
pub struct VideoFrame {
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    pub data: Vec<u8>,
}

/// C callback invoked (on the GStreamer streaming thread) when a new frame is
/// ready. The native texture layer typically marks its Flutter texture frame
/// available; it must be cheap and thread-safe.
pub type FrameReadyFn = extern "C" fn(ctx: *mut c_void);

struct FrameCallback {
    ctx: usize,
    func: FrameReadyFn,
}

// SAFETY: `ctx` is an opaque pointer owned by the native texture layer and only
// handed back to `func`. The native side must unregister before freeing it.
unsafe impl Send for FrameCallback {}

/// Latest-frame double buffer plus an optional native notify callback.
#[derive(Default)]
pub struct FrameSink {
    latest: Mutex<Option<VideoFrame>>,
    callback: Mutex<Option<FrameCallback>>,
}

impl FrameSink {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Registers the native notify callback (replaces any previous one).
    pub fn set_callback(&self, ctx: *mut c_void, func: FrameReadyFn) {
        *self.callback.lock() = Some(FrameCallback {
            ctx: ctx as usize,
            func,
        });
    }

    pub fn clear_callback(&self) {
        *self.callback.lock() = None;
    }

    fn store_frame(&self, frame: VideoFrame) {
        *self.latest.lock() = Some(frame);
        // Notify without holding the frame lock, so the native pull cannot
        // deadlock against the producer.
        let notify = {
            let guard = self.callback.lock();
            guard.as_ref().map(|c| (c.ctx, c.func))
        };
        if let Some((ctx, func)) = notify {
            func(ctx as *mut c_void);
        }
    }

    /// Copies the latest frame's BGRA pixels into `dst`. Returns
    /// `(width, height, stride)` or `None` if there is no frame or `dst` is too
    /// small for the frame's byte length.
    pub fn copy_latest(&self, dst: &mut [u8]) -> Option<(i32, i32, i32)> {
        let guard = self.latest.lock();
        let frame = guard.as_ref()?;
        if dst.len() < frame.data.len() {
            return None;
        }
        dst[..frame.data.len()].copy_from_slice(&frame.data);
        Some((frame.width, frame.height, frame.stride))
    }

    /// Latest frame geometry: `(width, height, stride, byte_len)`.
    pub fn latest_geometry(&self) -> Option<(i32, i32, i32, usize)> {
        let guard = self.latest.lock();
        let frame = guard.as_ref()?;
        Some((frame.width, frame.height, frame.stride, frame.data.len()))
    }
}

/// Builds an `appsink` that outputs BGRA and feeds `sink` on every frame.
///
/// `max-buffers=1` + `drop=true` keeps latency low (always render the newest
/// frame); `sync=true` respects pipeline clock pacing.
///
/// Wired into `create_platform_video_sink` on the appsink platforms (iOS today;
/// macOS/Windows/Linux in later phases).
#[cfg_attr(
    not(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    )),
    allow(dead_code)
)]
pub fn build_frame_appsink(sink: Arc<FrameSink>) -> Result<gst::Element> {
    let appsink = gst::ElementFactory::make("appsink")
        .build()
        .map_err(|_| anyhow!("failed to create appsink"))?
        .dynamic_cast::<AppSink>()
        .map_err(|_| anyhow!("element is not AppSink"))?;

    let caps = gst_video::VideoCapsBuilder::new()
        .format(FRAME_FORMAT_BGRA)
        .build();
    appsink.set_caps(Some(&caps));
    appsink.set_max_buffers(1);
    appsink.set_drop(true);
    appsink.set_property("sync", true);

    appsink.set_callbacks(
        AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                if let Some(frame) = sample_to_frame(&sample) {
                    sink.store_frame(frame);
                }
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    Ok(appsink.upcast())
}

fn sample_to_frame(sample: &gst::Sample) -> Option<VideoFrame> {
    let buffer = sample.buffer()?;
    let caps = sample.caps()?;
    let info = gst_video::VideoInfo::from_caps(caps).ok()?;
    let vframe = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info).ok()?;
    let stride = vframe.plane_stride()[0];
    let width = vframe.width() as i32;
    let height = vframe.height() as i32;
    let data = vframe.plane_data(0).ok()?.to_vec();
    Some(VideoFrame {
        width,
        height,
        stride,
        data,
    })
}

/// Process-wide `player_id -> FrameSink` registry so the native texture C-ABI
/// can reach a player's frame source (the pipeline is built before the player
/// id is assigned, so the engine registers here after creation).
static FRAME_SINKS: Lazy<Mutex<HashMap<i64, Arc<FrameSink>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register_frame_sink(player_id: i64, sink: Arc<FrameSink>) {
    FRAME_SINKS.lock().insert(player_id, sink);
}

pub fn unregister_frame_sink(player_id: i64) {
    FRAME_SINKS.lock().remove(&player_id);
}

pub fn frame_sink_for(player_id: i64) -> Option<Arc<FrameSink>> {
    FRAME_SINKS.lock().get(&player_id).cloned()
}
