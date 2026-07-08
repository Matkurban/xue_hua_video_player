//! appsink 视频帧源，供 Flutter 外部纹理渲染 / appsink-based video frame source for Flutter external-texture rendering.
//!
//! 在 Apple / Windows / Linux 上管线以 `appsink` 终结，输出 `BGRA` 帧；每帧复制到
//! 最新帧双缓冲并通知原生纹理层，原生侧经 [`FrameSink::copy_latest`] 拉取像素更新
//! Flutter 外部纹理。Android 使用 `glimagesink` 渲染到纹理 `Surface`，不使用本模块。
//!
//! Dart → [`crate::playback::engine::PlaybackEngine`] → [`crate::playback::gst::create_platform_video_sink`]
//! → 本模块 `appsink` → 原生 C-ABI 纹理桥。
//!
//! On Apple / Windows / Linux the pipeline terminates in an `appsink` producing `BGRA` frames.
//! Each decoded frame is copied into a latest-frame double buffer and the registered native
//! texture layer is notified; the native side pulls pixels via [`FrameSink::copy_latest`] and
//! updates the Flutter external texture. Android uses `glimagesink` into a texture-backed
//! `Surface` and does not use this module.

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

/// 交付给原生纹理层的像素格式（与 appsink caps 一致）/ Pixel format delivered to the native texture layer.
pub const FRAME_FORMAT_BGRA: gst_video::VideoFormat = gst_video::VideoFormat::Bgra;

/// 解码后的 BGRA 帧；`stride` 为行字节步长（可能大于 `width * 4`）/ A decoded BGRA frame; `stride` is row stride in bytes.
pub struct VideoFrame {
    /// 像素宽度 / Pixel width.
    pub width: i32,
    /// 像素高度 / Pixel height.
    pub height: i32,
    /// 行步长（字节）/ Row stride in bytes.
    pub stride: i32,
    /// 像素数据 / Pixel buffer.
    pub data: Vec<u8>,
}

/// GStreamer streaming 线程上调用的 C 回调：新帧就绪时通知原生层 / C callback on GStreamer streaming thread when a new frame is ready.
pub type FrameReadyFn = extern "C" fn(ctx: *mut c_void);

struct FrameCallback {
    ctx: usize,
    func: FrameReadyFn,
}

// SAFETY: `ctx` is an opaque pointer owned by the native texture layer and only
// handed back to `func`. The native side must unregister before freeing it.
unsafe impl Send for FrameCallback {}

/// 最新帧双缓冲 + 可选原生通知回调 / Latest-frame double buffer plus optional native notify callback.
#[derive(Default)]
pub struct FrameSink {
    latest: Mutex<Option<VideoFrame>>,
    callback: Mutex<Option<FrameCallback>>,
}

impl FrameSink {
    /// 创建新的共享 [`FrameSink`] / Creates a new shared [`FrameSink`].
    ///
    /// # 参数 / Parameters
    /// - 无 / None
    ///
    /// # 返回值 / Returns
    /// - `Arc<FrameSink>` / shared frame sink
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 任意线程 / any thread
    ///
    /// # 平台 / Platform
    /// - appsink 平台（iOS/macOS/Windows/Linux）/ appsink platforms
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 注册原生通知回调（替换已有回调）/ Registers the native notify callback (replaces any previous one).
    ///
    /// # 参数 / Parameters
    /// - `ctx` — 不透明原生上下文指针 / opaque native context pointer
    /// - `func` — 帧就绪 C 回调 / frame-ready C callback
    ///
    /// # 返回值 / Returns
    /// - 无 / None
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 任意线程注册；`func` 在 streaming 线程调用 / register on any thread; `func` called on streaming thread
    ///
    /// # 平台 / Platform
    /// - 原生纹理 C-ABI 桥 / native texture C-ABI bridge
    pub fn set_callback(&self, ctx: *mut c_void, func: FrameReadyFn) {
        *self.callback.lock() = Some(FrameCallback {
            ctx: ctx as usize,
            func,
        });
    }

    /// 清除已注册回调 / Clears the registered callback.
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

    /// 将最新帧 BGRA 像素复制到 `dst` / Copies the latest frame's BGRA pixels into `dst`.
    ///
    /// # 参数 / Parameters
    /// - `dst` — 目标缓冲区 / destination buffer
    ///
    /// # 返回值 / Returns
    /// - 成功：`(width, height, stride)`；无帧或 `dst` 过小则 `None` / geometry or `None`
    ///
    /// # 错误 / Errors
    /// - 无（以 `None` 表示失败）/ None (returns `None` on failure)
    ///
    /// # 线程 / Threading
    /// - 任意线程；与 `store_frame` 通过互斥锁同步 / any thread; synchronized via mutex
    ///
    /// # 平台 / Platform
    /// - 原生纹理拉取路径 / native texture pull path
    pub fn copy_latest(&self, dst: &mut [u8]) -> Option<(i32, i32, i32)> {
        let guard = self.latest.lock();
        let frame = guard.as_ref()?;
        if dst.len() < frame.data.len() {
            return None;
        }
        dst[..frame.data.len()].copy_from_slice(&frame.data);
        Some((frame.width, frame.height, frame.stride))
    }

    /// 最新帧几何信息：`(width, height, stride, byte_len)` / Latest frame geometry.
    pub fn latest_geometry(&self) -> Option<(i32, i32, i32, usize)> {
        let guard = self.latest.lock();
        let frame = guard.as_ref()?;
        Some((frame.width, frame.height, frame.stride, frame.data.len()))
    }
}

/// 构建输出 BGRA 的 `appsink` 并喂入 `sink` / Builds an `appsink` that outputs BGRA and feeds `sink`.
///
/// `max-buffers=1` + `drop=true` 保持低延迟；`sync=true` 尊重管线时钟。
///
/// # 参数 / Parameters
/// - `sink` — 目标 [`FrameSink`] / target frame sink
///
/// # 返回值 / Returns
/// - 成功：appsink 元素 / appsink element
///
/// # 错误 / Errors
/// - appsink 创建或类型转换失败 / creation or cast failure
///
/// # 线程 / Threading
/// - 构建于 Gst 线程；回调在 streaming 线程 / built on Gst thread; callbacks on streaming thread
///
/// # 平台 / Platform
/// - 由 [`create_platform_video_sink`] 在 appsink 平台调用 / called from platform sink factory
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

/// 进程级 `player_id → FrameSink` 注册表，供原生纹理 C-ABI 寻址 / Process-wide registry for native texture C-ABI.
static FRAME_SINKS: Lazy<Mutex<HashMap<i64, Arc<FrameSink>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// 在 `create_player` 后注册帧源 / Registers frame source after `create_player`.
///
/// # 参数 / Parameters
/// - `player_id` — FRB 播放器 ID / FRB player id
/// - `sink` — 引擎 [`FrameSink`] / engine frame sink
///
/// # 返回值 / Returns
/// - 无 / None
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - API 层调用 / API layer
///
/// # 平台 / Platform
/// - appsink 平台 / appsink platforms
pub fn register_frame_sink(player_id: i64, sink: Arc<FrameSink>) {
    FRAME_SINKS.lock().insert(player_id, sink);
}

/// 注销帧源（dispose 时）/ Unregisters frame source on dispose.
pub fn unregister_frame_sink(player_id: i64) {
    FRAME_SINKS.lock().remove(&player_id);
}

/// 按 `player_id` 查找帧源 / Looks up frame sink by `player_id`.
pub fn frame_sink_for(player_id: i64) -> Option<Arc<FrameSink>> {
    FRAME_SINKS.lock().get(&player_id).cloned()
}
