//! iOS `avsamplebufferlayersink` CALayer 附着与预卷逻辑 /
//! iOS `avsamplebufferlayersink` CALayer attach and preroll logic.
//!
//! 从 sink 读取 `layer` 属性、按需预卷管线至 Paused、在主线程异步附着 CALayer，
//! 并通过 completion 回调报告结果给 [`IosOverlaySession`]。
//!
//! Reads the sink `layer` property, prerolls the pipeline to Paused when needed,
//! async-attaches CALayer on the main thread, and reports outcomes to [`IosOverlaySession`] via completion.

use std::ffi::CStr;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;

#[cfg(target_os = "ios")]
use crate::platform::ios::{attach_layer_on_main_thread_async, host_view_ready_for_attach};
use crate::playback::shell::set_element_state_sync;
use crate::playback::shell::PipelineShell;

#[cfg(target_os = "ios")]
extern "C" {
    fn CFRetain(cf: *const std::ffi::c_void) -> *const std::ffi::c_void;
}

/// 尝试调度 iOS CALayer 附着的结果 / Outcome of attempting to schedule an iOS CALayer attach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IosLayerAttachOutcome {
    /// CALayer 附着已完成（同步路径）或已异步调度等待 re-layout /
    /// CALayer attach completed (sync path) or was scheduled async for re-layout.
    Scheduled,
    /// sink `layer` 属性尚未就绪；在总线 `READY→PAUSED` / `AsyncDone` 或布局时重试 /
    /// Sink `layer` property not ready yet; retry on bus `READY→PAUSED` / `AsyncDone` or layout.
    LayerNotReady,
    /// 无宿主视图可附着 / No host view to attach to.
    Skipped,
}

/// 释放 [`read_sink_layer`] 的 +1 retain（附着跳过或延迟时）/
/// Releases +1 retain from [`read_sink_layer`] when attach is skipped or deferred.
///
/// `CFRelease` 被调度到主线程：释放最后引用会 dealloc `AVSampleBufferDisplayLayer`，
/// 必须在主线程进行（Apple 要求所有 display-layer 操作在主线程）。
///
/// The `CFRelease` is marshaled to the main thread: releasing the last reference
/// deallocates the `AVSampleBufferDisplayLayer`, which must happen on the main
/// thread (Apple requires all display-layer operations there).
///
/// # 参数 / Parameters
/// - `layer` — 由 [`read_sink_layer`] 返回的 +1 retain 指针 / +1 retained pointer from [`read_sink_layer`]
#[cfg(target_os = "ios")]
pub fn release_sink_layer(layer: usize) {
    crate::platform::ios::release_layer_on_main_thread(layer);
}

#[cfg(not(target_os = "ios"))]
pub fn release_sink_layer(_layer: usize) {}

/// 从 `avsamplebufferlayersink` 读取 `layer` 属性（CALayer 指针）/
/// Reads the `layer` property from `avsamplebufferlayersink` (CALayer pointer).
///
/// iOS 上返回的指针为 +1 retain；通过附着 shim 或 [`release_sink_layer`] 释放。
///
/// On iOS the returned pointer is +1 retained; release via attach shim or `release_sink_layer`.
///
/// # 参数 / Parameters
/// - `sink` — `avsamplebufferlayersink` 元素 / `avsamplebufferlayersink` element
///
/// # 返回值 / Returns
/// - `Ok(layer)` CALayer 指针整型表示 / CALayer pointer as integer
///
/// # 错误 / Errors
/// - `layer` 属性为 null（尚未就绪）/ `layer` property is null (not ready yet)
pub fn read_sink_layer(sink: &gst::Element) -> Result<usize> {
    unsafe {
        let mut layer: glib::ffi::gpointer = std::ptr::null_mut();
        glib::gobject_ffi::g_object_get(
            sink.as_ptr() as *mut glib::gobject_ffi::GObject,
            CStr::from_bytes_with_nul(b"layer\0")
                .map_err(|_| anyhow!("layer property name"))?
                .as_ptr(),
            &mut layer,
            std::ptr::null_mut::<glib::ffi::gpointer>(),
        );
        if layer.is_null() {
            return Err(anyhow!("sink layer not ready yet"));
        }
        #[cfg(target_os = "ios")]
        {
            let retained = CFRetain(layer as *const std::ffi::c_void);
            Ok(retained as usize)
        }
        #[cfg(not(target_os = "ios"))]
        {
            Ok(layer as usize)
        }
    }
}

/// 有待加载媒体时将 `pipeline` 预卷至 PAUSED（iOS Tutorial 4 附着流程）/
/// Prerolls `pipeline` to PAUSED when media is pending (iOS Tutorial 4 attach flow).
///
/// 调用时勿持有 `PipelineShell` 互斥锁 — 总线处理器可能重入附着。
///
/// Call without holding `PipelineShell` mutex — bus handlers may re-enter attach.
///
/// # 参数 / Parameters
/// - `pipeline` — GStreamer 管线 / GStreamer pipeline
///
/// # 返回值 / Returns
/// - `Ok(())` 预卷完成或无需预卷 / `Ok(())` when preroll completes or is unnecessary
pub fn preroll_pipeline_for_ios_layer(pipeline: &gst::Pipeline) -> Result<()> {
    let (_, mut current, mut pending) = pipeline.state(gst::ClockTime::ZERO);
    log::info!("gst: ios preroll enter current={current:?} pending={pending:?}");
    if current < gst::State::Ready {
        set_element_state_sync(pipeline, gst::State::Ready)?;
        (_, current, pending) = pipeline.state(gst::ClockTime::ZERO);
    }

    if current == gst::State::Ready && pending == gst::State::VoidPending {
        set_element_state_sync(pipeline, gst::State::Paused)?;
        let (_, after, _) = pipeline.state(gst::ClockTime::ZERO);
        log::info!("gst: ios preroll reached {after:?}");
    }
    Ok(())
}

/// CALayer 附着无法完成时回滚 Paused 预卷（无 layer 时停止解码）/
/// Reverts a PAUSED preroll when CALayer attach cannot complete (stops decode without layer).
#[cfg(target_os = "ios")]
fn rollback_pipeline_from_ios_preroll(pipeline: &gst::Pipeline) {
    let (_, current, _) = pipeline.state(gst::ClockTime::ZERO);
    if current >= gst::State::Paused {
        if let Err(e) = set_element_state_sync(pipeline, gst::State::Ready) {
            log::warn!("gst: ios layer attach rollback to READY: {e:#}");
        } else {
            log::info!("gst: ios layer attach preroll rolled back to READY");
        }
    }
}

#[cfg(not(target_os = "ios"))]
fn rollback_pipeline_from_ios_preroll(_pipeline: &gst::Pipeline) {}

/// 将管线预卷至 PAUSED（有待加载媒体时）/ Prerolls the pipeline to PAUSED when media is pending.
///
/// # 参数 / Parameters
/// - `shell` — 管线壳层 / pipeline shell
///
/// # 返回值 / Returns
/// - `Ok(())` 成功；非 iOS 为 no-op / `Ok(())` on success; no-op off iOS
#[cfg(target_os = "ios")]
pub fn preroll_for_ios_layer(shell: &PipelineShell) -> Result<()> {
    shell.preroll_for_ios_layer()
}

#[cfg(not(target_os = "ios"))]
pub fn preroll_for_ios_layer(_shell: &PipelineShell) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "ios")]
fn schedule_layer_attach_async<F>(
    host_view: usize,
    layer: usize,
    pipeline: gst::Pipeline,
    prerolled: bool,
    on_complete: F,
) -> IosLayerAttachOutcome
where
    F: FnOnce(bool) + Send + 'static,
{
    attach_layer_on_main_thread_async(host_view, layer, move |attached| {
        if !attached {
            release_sink_layer(layer);
            if prerolled {
                rollback_pipeline_from_ios_preroll(&pipeline);
            }
            log::debug!(
                "gst: ios layer attach async deferred host={host_view:#x} (zero bounds or verify failed)"
            );
        } else {
            log::info!("gst: ios layer attached async host={host_view:#x} layer={layer:#x}");
        }
        on_complete(attached);
    });
    IosLayerAttachOutcome::Scheduled
}

/// 按需预卷、在主线程异步附着 sink CALayer，并通过回调报告成功与否 /
/// Prerolls when needed, async-attaches the sink CALayer on the main thread, then reports success.
///
/// # 参数 / Parameters
/// - `pipeline` — GStreamer 管线 / GStreamer pipeline
/// - `has_pending_media` — 是否已有待加载媒体 / whether media is pending
/// - `sink` — `avsamplebufferlayersink` 元素 / `avsamplebufferlayersink` element
/// - `host_view` — Flutter 宿主 `UIView` 指针 / Flutter host `UIView` pointer
/// - `on_complete` — Gst 线程上的附着结果回调（`true` 成功）/ attach result callback on Gst thread (`true` on success)
///
/// # 返回值 / Returns
/// - `Ok(outcome)` [`IosLayerAttachOutcome`] 结果 / [`IosLayerAttachOutcome`] result
///
/// # 错误 / Errors
/// - 无待加载媒体且 `read_sink_layer` 失败 / `read_sink_layer` failure with no pending media
#[cfg(target_os = "ios")]
pub fn attach_ios_video_layer_with_completion<F>(
    pipeline: &gst::Pipeline,
    has_pending_media: bool,
    sink: &gst::Element,
    host_view: usize,
    on_complete: F,
) -> Result<IosLayerAttachOutcome>
where
    F: FnOnce(bool) + Send + 'static,
{
    if host_view == 0 {
        return Ok(IosLayerAttachOutcome::Skipped);
    }

    if !host_view_ready_for_attach(host_view) {
        log::debug!("gst: ios layer attach deferred host={host_view:#x} (zero bounds)");
        return Ok(IosLayerAttachOutcome::LayerNotReady);
    }

    let pipeline_owned = pipeline.clone();

    if !has_pending_media {
        return match read_sink_layer(sink) {
            Ok(layer) => Ok(schedule_layer_attach_async(
                host_view,
                layer,
                pipeline_owned,
                false,
                on_complete,
            )),
            Err(e) => Err(anyhow!("sink layer not ready yet (no pending media): {e}")),
        };
    }

    preroll_pipeline_for_ios_layer(pipeline)?;
    let prerolled = true;

    match read_sink_layer(sink) {
        Ok(layer) => {
            log::info!("gst: ios read_sink_layer ok after preroll layer={layer:#x}");
            Ok(schedule_layer_attach_async(
                host_view,
                layer,
                pipeline_owned,
                prerolled,
                on_complete,
            ))
        }
        Err(e) => {
            rollback_pipeline_from_ios_preroll(pipeline);
            log::debug!("gst: ios read_sink_layer after preroll: {e:#}");
            Ok(IosLayerAttachOutcome::LayerNotReady)
        }
    }
}

#[cfg(not(target_os = "ios"))]
pub fn attach_ios_video_layer_with_completion<F>(
    _pipeline: &gst::Pipeline,
    _has_pending_media: bool,
    _sink: &gst::Element,
    _host_view: usize,
    _on_complete: F,
) -> Result<IosLayerAttachOutcome>
where
    F: FnOnce(bool) + Send + 'static,
{
    Ok(IosLayerAttachOutcome::Skipped)
}
