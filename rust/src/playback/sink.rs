//! 音频/字幕 sink bin 与视频探针 / Audio/subtitle sink bins and video pad probe.
//!
//! 为 playbin 与 AppSrc 管线构建音频 scaletempo bin、HTTP 源配置、视频 caps 探针
//! （发射尺寸/元数据事件），以及 Android overlay 尺寸同步回调。
//!
//! Builds audio scaletempo bins, HTTP source configuration, and video caps probes
//! (emitting size/metadata events) for playbin and AppSrc pipelines, plus Android
//! overlay size sync callbacks.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video as gst_video;
use parking_lot::Mutex;

use crate::playback::bus::Emitter;
use crate::playback::gst::{expose_overlay, InternalVideoMetadata};

/// Android 专用：解码视频尺寸变更时调用（caps 协商后）/ Android-only: invoked when decoded video dimensions change.
#[cfg(target_os = "android")]
pub type OverlaySizeSync = Arc<dyn Fn(i32, i32) + Send + Sync>;

/// 配置 HTTP(S) 源元素的 TLS 与 User-Agent / Configures TLS and user-agent on HTTP(S) source elements.
///
/// # 参数 / Parameters
/// - `element` — `souphttpsrc` 等源元素 / source element such as `souphttpsrc`
///
/// # 返回值 / Returns
/// - 无 / None
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - 在 playbin `source-setup`/`element-setup` 回调中调用 / called from playbin setup callbacks
///
/// # 平台 / Platform
/// - 网络 URI 源；移动 UA 字符串 / network URI sources; mobile user-agent string
pub fn configure_http_source(element: &gst::Element) {
    let is_souphttpsrc = element
        .factory()
        .is_some_and(|f| f.name().as_str() == "souphttpsrc");

    if element.find_property("ssl-strict").is_some() {
        element.set_property("ssl-strict", false);
    }
    if element.find_property("tls-validation-flags").is_some() {
        // GIO_TLS_CERTIFICATE_VALIDATE_ALL = 0x7f (permissive when combined with ssl-strict=false).
        element.set_property("tls-validation-flags", 0u32);
    }
    if element.find_property("user-agent").is_some() {
        element.set_property(
            "user-agent",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148",
        );
    }

    if is_souphttpsrc && element.find_property("http-status-code").is_some() {
        element.connect_notify(Some("http-status-code"), move |el, _| {
            let code: u32 = el.property("http-status-code");
            if !(200..300).contains(&code) {
                log::warn!("souphttpsrc HTTP status code: {code}");
                #[cfg(target_os = "android")]
                crate::diag::logcat_error(&format!("souphttpsrc HTTP status code: {code}"));
            } else {
                log::debug!("souphttpsrc HTTP status code: {code}");
            }
        });
    }
}

/// 构建带可选 `scaletempo` 的音频 sink bin（保 pitch 变速）/ Builds audio sink bin with optional `scaletempo` for pitch-preserving rate changes.
///
/// # 参数 / Parameters
/// - 无 / None
///
/// # 返回值 / Returns
/// - 成功：带 ghost pad 的 [`gst::Bin`] / `gst::Bin` with ghost pad
///
/// # 错误 / Errors
/// - `autoaudiosink` 创建失败 / autoaudiosink creation failure
///
/// # 线程 / Threading
/// - 管线构建阶段于 Gst 线程 / pipeline build on Gst thread
///
/// # 平台 / Platform
/// - `scaletempo` 不可用时回退直连 audiosink / falls back without scaletempo if unavailable
pub fn build_audio_sink_bin() -> Result<gst::Bin> {
    let audio_bin = gst::Bin::new();
    let audiosink = gst::ElementFactory::make("autoaudiosink")
        .build()
        .map_err(|_| anyhow!("failed to create autoaudiosink"))?;

    let head = match (
        gst::ElementFactory::make("scaletempo").build(),
        gst::ElementFactory::make("audioconvert").build(),
        gst::ElementFactory::make("audioresample").build(),
    ) {
        (Ok(scaletempo), Ok(audioconvert), Ok(audioresample)) => {
            audio_bin.add(&scaletempo)?;
            audio_bin.add(&audioconvert)?;
            audio_bin.add(&audioresample)?;
            audio_bin.add(&audiosink)?;
            scaletempo.link(&audioconvert)?;
            audioconvert.link(&audioresample)?;
            audioresample.link(&audiosink)?;
            scaletempo
        }
        _ => {
            log::warn!("scaletempo unavailable: playback speed may change pitch");
            audio_bin.add(&audiosink)?;
            audiosink
        }
    };

    let sink_pad = head
        .static_pad("sink")
        .ok_or_else(|| anyhow!("audio sink head has no sink pad"))?;
    let ghost = gst::GhostPad::with_target(&sink_pad)?;
    ghost.set_active(true)?;
    audio_bin.add_pad(&ghost)?;

    Ok(audio_bin)
}

/// 构建 fakesink 字幕 bin，使 playbin 暴露字幕轨元数据但不渲染 / Builds fakesink text bin for subtitle track metadata without rendering.
///
/// # 参数 / Parameters
/// - 无 / None
///
/// # 返回值 / Returns
/// - 成功：字幕 sink bin / subtitle sink bin
///
/// # 错误 / Errors
/// - `fakesink` 创建失败 / fakesink creation failure
///
/// # 线程 / Threading
/// - 管线构建阶段 / pipeline build phase
///
/// # 平台 / Platform
/// - 仅 playbin URI 管线 / playbin URI pipelines only
pub fn build_text_sink_bin() -> Result<gst::Bin> {
    let text_bin = gst::Bin::new();
    let fakesink = gst::ElementFactory::make("fakesink")
        .build()
        .map_err(|_| anyhow!("failed to create fakesink for text"))?;
    text_bin.add(&fakesink)?;
    let sink_pad = fakesink
        .static_pad("sink")
        .ok_or_else(|| anyhow!("fakesink has no sink pad"))?;
    let ghost = gst::GhostPad::with_target(&sink_pad)?;
    ghost.set_active(true)?;
    text_bin.add_pad(&ghost)?;
    Ok(text_bin)
}

/// 在视频 sink pad 上挂探针：解码尺寸变更时发射事件与元数据 / Attaches pad probe to emit size/metadata on dimension changes.
///
/// # 参数 / Parameters
/// - `video_sink` — 平台视频 sink / platform video sink
/// - `emitter` — 事件发射器 / event emitter
/// - `metadata_cache` — 可选元数据缓存 / optional metadata cache
/// - `overlay_size_sync`（Android）— 尺寸同步回调 / size sync callback on Android
///
/// # 返回值 / Returns
/// - 无（无 sink pad 时静默返回）/ None (no-op if no sink pad)
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - 探针在 GStreamer streaming 线程触发 / probe runs on GStreamer streaming thread
///
/// # 平台 / Platform
/// - Android：触发 overlay 尺寸同步；非 Apple 平台首帧时 `expose_overlay`
pub fn attach_video_probe(
    video_sink: &gst::Element,
    emitter: Arc<Mutex<Option<Emitter>>>,
    metadata_cache: Option<Arc<Mutex<InternalVideoMetadata>>>,
    #[cfg(target_os = "android")] overlay_size_sync: Option<OverlaySizeSync>,
) {
    let sink_pad = match video_sink.static_pad("sink") {
        Some(pad) => pad,
        None => return,
    };
    let last_size = Arc::new(Mutex::new((0i32, 0i32)));
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    let sink_for_expose = video_sink.clone();
    #[cfg(target_os = "android")]
    let overlay_size_sync = overlay_size_sync;
    sink_pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
        if let Some(gst::PadProbeData::Event(ref ev)) = info.data {
            if let gst::EventView::Caps(caps) = ev.view() {
                if let Ok(video_info) = gst_video::VideoInfo::from_caps(caps.caps()) {
                    let width = video_info.width() as i32;
                    let height = video_info.height() as i32;
                    let mut ls = last_size.lock();
                    if *ls != (width, height) {
                        let first = ls.0 == 0 && ls.1 == 0;
                        if first {
                            // Diagnostic: negotiated sink caps reveal the pixel
                            // format and any memory feature (e.g. system memory vs
                            // IOSurface/CVPixelBuffer), which decides whether iOS
                            // `avsamplebufferlayersink` can render on device.
                            log::info!("gst: video sink negotiated caps: {}", caps.caps());
                        }
                        *ls = (width, height);
                        if let Some(cb) = emitter.lock().as_ref() {
                            use crate::player_events::PlayerEvent;
                            cb(PlayerEvent::video_size(width, height));
                            let meta = InternalVideoMetadata::from_video_info_and_caps(
                                &video_info,
                                Some(caps.caps()),
                            );
                            if let Some(cache) = metadata_cache.as_ref() {
                                *cache.lock() = meta.clone();
                            }
                            cb(PlayerEvent::metadata(meta));
                        }
                        #[cfg(target_os = "android")]
                        if width > 0 && height > 0 {
                            if let Some(sync) = overlay_size_sync.as_ref() {
                                sync(width, height);
                            }
                        }
                        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
                        if first && width > 0 && height > 0 {
                            expose_overlay(&sink_for_expose);
                        }
                    }
                }
            }
        }
        gst::PadProbeReturn::Ok
    });
}

/// 构建 Android overlay 尺寸同步闭包（纹理 content size + rectangle sync）/ Builds Android overlay size sync closure.
///
/// # 参数 / Parameters
/// - `player_id` — FRB 播放器 ID / FRB player id
/// - `gst_context` — 可选 [`PlaybackGstContext`] 槽（创建后填充）/ optional context slot filled after creation
///
/// # 返回值 / Returns
/// - [`OverlaySizeSync`] 回调 / sync callback
///
/// # 错误 / Errors
/// - 回调内部错误仅记录日志 / errors inside callback are logged only
///
/// # 线程 / Threading
/// - 在 streaming 线程调用；调度 mobile overlay rectangle sync / called on streaming thread
///
/// # 平台 / Platform
/// - 仅 Android / Android only
#[cfg(target_os = "android")]
pub fn android_overlay_size_sync(
    player_id: Arc<std::sync::atomic::AtomicI64>,
    gst_context: Arc<Mutex<Option<Arc<crate::playback::gst_context::PlaybackGstContext>>>>,
) -> OverlaySizeSync {
    use std::sync::atomic::Ordering;

    Arc::new(move |width, height| {
        if width < 2 || height < 2 {
            return;
        }
        let id = player_id.load(Ordering::SeqCst);
        if id <= 0 {
            return;
        }
        if let Err(e) = crate::platform::android::notify_texture_content_size(id, width, height) {
            crate::diag::logcat_error(&format!("android setContentSize: {e:#}"));
        }
        if let Some(ctx) = gst_context.lock().clone() {
            ctx.surface.set_cached_dimensions(width, height);
            ctx.surface
                .schedule_mobile_overlay_rectangle_sync(ctx.shell.clone(), width, height);
        }
    })
}
