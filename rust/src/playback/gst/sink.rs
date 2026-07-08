//! 平台视频 sink 与 VideoOverlay 绑定 / Platform video sink and VideoOverlay binding.
//!
//! 按目标 OS 创建 appsink 或 VideoOverlay sink，处理 `prepare-window-handle` 总线同步、
//! 窗口句柄绑定与渲染矩形更新；是 xhvp-gst 输出到 Flutter 纹理或原生视图的最后一环。
//!
//! Creates appsink or VideoOverlay sinks per target OS, handles `prepare-window-handle`
//! bus sync, window handle binding, and render rectangle updates — the final xhvp-gst
//! output stage to Flutter textures or native views.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video::{
    self as gst_video,
    prelude::{VideoOverlayExt, VideoOverlayExtManual},
};
use parking_lot::Mutex;

use super::orientation::make_videoflip_element;

/// 各平台 GStreamer 推荐的视频 sink 元素名 / GStreamer-recommended video sink element name per platform.
#[cfg_attr(
    any(
        target_os = "ios",
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    ),
    allow(dead_code)
)]
pub fn video_sink_factory_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "d3d11videosink"
    }
    #[cfg(target_os = "macos")]
    {
        "osxvideosink"
    }
    #[cfg(target_os = "ios")]
    {
        "avsamplebufferlayersink"
    }
    #[cfg(all(
        not(target_os = "windows"),
        not(target_os = "macos"),
        not(target_os = "ios")
    ))]
    {
        "glimagesink"
    }
}

#[cfg(not(any(
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    target_os = "linux"
)))]
fn configure_video_sink(element: &gst::Element) {
    if element.find_property("force-aspect-ratio").is_some() {
        element.set_property("force-aspect-ratio", true);
    }
}

/// 为当前平台创建视频 sink / Creates the video sink for the current platform.
///
/// Apple（iOS/macOS）与桌面（Windows/Linux）经 Flutter 外部纹理，以 `appsink`（BGRA）
/// 喂入 `frame_sink`；Android 仍使用 VideoOverlay sink（`glimagesink`）。
///
/// # 参数 / Parameters
/// - `frame_sink` — 外部纹理帧源（appsink 平台）/ frame source for appsink platforms
///
/// # 返回值 / Returns
/// - 成功：视频 sink 元素 / video sink element
///
/// # 错误 / Errors
/// - 元素工厂创建失败 / element factory failure
///
/// # 线程 / Threading
/// - 管线构建于 Gst 线程 / pipeline build on Gst thread
///
/// # 平台 / Platform
/// - appsink：iOS/macOS/Windows/Linux；VideoOverlay：Android 等 / per-OS sink selection
#[cfg_attr(
    not(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    )),
    allow(unused_variables)
)]
pub fn create_platform_video_sink(
    frame_sink: &std::sync::Arc<crate::playback::frame::FrameSink>,
) -> Result<gst::Element> {
    #[cfg(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    ))]
    {
        return crate::playback::frame::build_frame_appsink(frame_sink.clone());
    }
    #[cfg(not(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    )))]
    {
        let name = video_sink_factory_name();
        let sink = gst::ElementFactory::make(name)
            .build()
            .map_err(|e| anyhow!("failed to create {name}: {e}"))?;
        configure_video_sink(&sink);
        Ok(sink)
    }
}

/// 构建含 `videoflip` 的 playbin `video-sink` bin（NULL 态安装）/ Builds playbin `video-sink` bin with in-pipeline `videoflip`.
///
/// 链路：`videoflip → videoconvert →` 平台 sink（appsink 或 VideoOverlay）。
/// 返回 `(bin, inner_sink, videoflip)`；probe/overlay 仍挂 `inner_sink`。
pub fn build_video_sink_bin(
    frame_sink: &Arc<crate::playback::frame::FrameSink>,
) -> Result<(gst::Bin, gst::Element, gst::Element)> {
    let videoflip = make_videoflip_element()?;
    let videoconvert = gst::ElementFactory::make("videoconvert")
        .build()
        .map_err(|e| anyhow!("videoconvert: {e}"))?;
    let inner_sink = create_platform_video_sink(frame_sink)?;

    let bin = gst::Bin::new();
    bin.add_many([&videoflip, &videoconvert, &inner_sink])?;
    gst::Element::link_many([&videoflip, &videoconvert, &inner_sink])?;

    let sink_pad = videoflip
        .static_pad("sink")
        .ok_or_else(|| anyhow!("videoflip has no sink pad"))?;
    let ghost = gst::GhostPad::with_target(&sink_pad)?;
    ghost.set_active(true)?;
    bin.add_pad(&ghost)?;

    Ok((bin, inner_sink, videoflip))
}

/// 将原生窗口/surface 句柄绑定到 VideoOverlay sink / Binds native window/surface handle to VideoOverlay sink.
///
/// 不调用 `expose`——调用方应在首帧或已知渲染矩形后再 expose，避免绿屏闪烁。
///
/// # 参数 / Parameters
/// - `video_sink` — 视频 sink 元素 / video sink element
/// - `handle` — 原生句柄（整数）/ native handle as integer
///
/// # 返回值 / Returns
/// - 成功：`Ok(())` / `Ok(())`
///
/// # 错误 / Errors
/// - sink 未实现 VideoOverlay / sink does not implement VideoOverlay
///
/// # 线程 / Threading
/// - 必须在 Gst 线程（或平台文档允许的线程）/ Gst thread per platform docs
///
/// # 平台 / Platform
/// - VideoOverlay 平台（Android 等）/ VideoOverlay platforms
pub fn set_overlay_window_handle(video_sink: &gst::Element, handle: usize) -> Result<()> {
    let overlay = video_sink
        .clone()
        .dynamic_cast::<gst_video::VideoOverlay>()
        .map_err(|_| anyhow!("video sink does not implement VideoOverlay"))?;
    unsafe {
        overlay.set_window_handle(handle);
    }
    Ok(())
}

fn bind_overlay_element(overlay: &gst_video::VideoOverlay, handle: usize) {
    unsafe {
        overlay.set_window_handle(handle);
    }
}

/// 清除 overlay 窗口句柄（surface 销毁时）/ Clears overlay window handle when surface is destroyed.
pub fn clear_overlay_window_handle(video_sink: &gst::Element) -> Result<()> {
    set_overlay_window_handle(video_sink, 0)
}

/// 请求重绘（surface 几何变更后）/ Requests redraw after surface geometry changes.
///
/// Android 教程建议调用两次 `expose` 以应对 GLES 管线尺寸传播。
pub fn expose_overlay(video_sink: &gst::Element) {
    if let Ok(overlay) = video_sink.clone().dynamic_cast::<gst_video::VideoOverlay>() {
        overlay.expose();
        overlay.expose();
    }
}

/// 设置嵌入视图矩形并请求重绘 / Sets embedded view rectangle and requests redraw.
///
/// 勿在 iOS Gst 线程调用——iOS 尺寸由主线程 `EaglUIView` layout 处理。
///
/// # 参数 / Parameters
/// - `video_sink` — 视频 sink / video sink
/// - `width`、`height` — 渲染区域尺寸 / render area size
///
/// # 返回值 / Returns
/// - 无 / None
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - Gst 线程（iOS 除外）/ Gst thread except iOS
///
/// # 平台 / Platform
/// - iOS：应由主线程 layout 处理 / iOS handled on main thread
pub fn set_overlay_render_rectangle(video_sink: &gst::Element, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    if let Ok(overlay) = video_sink.clone().dynamic_cast::<gst_video::VideoOverlay>() {
        let _ = overlay.set_render_rectangle(0, 0, width, height);
        overlay.expose();
        overlay.expose();
    }
}

/// 处理总线 `prepare-window-handle` 同步消息 / Handles `prepare-window-handle` on pipeline bus sync handler.
///
/// # 参数 / Parameters
/// - `msg` — 总线消息 / bus message
/// - `cached_handle` — 缓存的原生句柄 / cached native handle
///
/// # 返回值 / Returns
/// - [`gst::BusSyncReply`]：Pass、Drop 或平台特定处理 / Pass, Drop, or platform-specific
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - Gst 总线 sync handler 线程 / Gst bus sync handler thread
///
/// # 平台 / Platform
/// - iOS/macOS 转发到专用处理；其他平台直接绑定 / Darwin delegates; others bind directly
pub fn bus_sync_reply_for_overlay_message(
    msg: &gst::MessageRef,
    cached_handle: Option<usize>,
) -> gst::BusSyncReply {
    use gstreamer_video::is_video_overlay_prepare_window_handle_message;

    if !is_video_overlay_prepare_window_handle_message(msg) {
        return gst::BusSyncReply::Pass;
    }
    let Some(handle) = cached_handle else {
        log::warn!("prepare-window-handle received but no overlay handle is cached yet");
        return gst::BusSyncReply::Pass;
    };
    #[cfg(target_os = "ios")]
    {
        return bus_sync_reply_for_ios_overlay(msg, Some(handle), None);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = handle;
        return gst::BusSyncReply::Pass;
    }
    #[cfg(all(not(target_os = "ios"), not(target_os = "macos")))]
    if let Some(src) = msg.src() {
        if let Ok(overlay) = src.clone().dynamic_cast::<gst_video::VideoOverlay>() {
            log::info!("prepare-window-handle: binding overlay handle {handle:#x}");
            bind_overlay_element(&overlay, handle);
        }
    }
    #[cfg(all(not(target_os = "ios"), not(target_os = "macos")))]
    gst::BusSyncReply::Drop
}

/// iOS `prepare-window-handle`：Pass——由 [`IosOverlaySession`] 异步 CALayer 附加 / iOS: Pass — async CALayer attach.
#[cfg(target_os = "ios")]
pub fn bus_sync_reply_for_ios_overlay(
    msg: &gst::MessageRef,
    _cached_handle: Option<usize>,
    _overlay_sink: Option<&Arc<Mutex<gst::Element>>>,
) -> gst::BusSyncReply {
    use gstreamer_video::is_video_overlay_prepare_window_handle_message;

    if !is_video_overlay_prepare_window_handle_message(msg) {
        return gst::BusSyncReply::Pass;
    }
    log::debug!("prepare-window-handle: ignored on iOS (IosOverlaySession handles CALayer attach)");
    gst::BusSyncReply::Pass
}

/// 安装总线 sync handler 以应答 VideoOverlay `prepare-window-handle` / Installs bus sync handler for VideoOverlay.
///
/// # 参数 / Parameters
/// - `pipeline` — GStreamer 管线 / pipeline
/// - `overlay_handle` — 共享缓存句柄 / shared cached handle
/// - `overlay_sink`（macOS/iOS）— 可选 sink 槽 / optional sink slot
///
/// # 返回值 / Returns
/// - 无 / None
///
/// # 错误 / Errors
/// - 无（无 bus 时静默返回）/ None (no-op if no bus)
///
/// # 线程 / Threading
/// - handler 在 Gst 线程 / handler on Gst thread
///
/// # 平台 / Platform
/// - 各平台 sync 策略不同 / per-platform sync strategy
pub fn attach_overlay_bus_sync_handler(
    pipeline: &gst::Pipeline,
    overlay_handle: Arc<Mutex<Option<usize>>>,
    #[cfg(target_os = "ios")] overlay_sink: Option<Arc<Mutex<gst::Element>>>,
) {
    let bus = match pipeline.bus() {
        Some(bus) => bus,
        None => return,
    };
    #[cfg(target_os = "ios")]
    let overlay_sink_bus = overlay_sink.clone();
    bus.set_sync_handler(move |_bus, msg| {
        #[cfg(target_os = "ios")]
        {
            let handle = *overlay_handle.lock();
            return bus_sync_reply_for_ios_overlay(msg, handle, overlay_sink_bus.as_ref());
        }
        #[cfg(not(target_os = "ios"))]
        {
            let handle = *overlay_handle.lock();
            bus_sync_reply_for_overlay_message(msg, handle)
        }
    });
}
