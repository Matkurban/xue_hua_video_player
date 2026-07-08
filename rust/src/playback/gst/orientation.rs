//! playbin 画面旋转与宽高比 / playbin orientation and aspect ratio.
//!
//! 通过 playbin `video-filter` 上的 `videoflip` 应用旋转/翻转，并通过 sink 属性
//! 控制 `force-aspect-ratio` 实现 Fit/Fill/Stretch 模式。
//!
//! Applies rotation/flip via `videoflip` on playbin `video-filter` and controls
//! `force-aspect-ratio` on the sink for Fit/Fill/Stretch modes.

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;

/// 经 playbin `video-filter`（videoflip）应用的画面方向 / Video orientation via playbin `video-filter` (videoflip).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct InternalVideoOrientationConfig {
    /// 水平翻转 / Horizontal flip.
    pub flip_horizontal: bool,
    /// 垂直翻转 / Vertical flip.
    pub flip_vertical: bool,
    /// 顺时针旋转角度：0、90、180 或 270 / Clockwise rotation: 0, 90, 180, or 270 degrees.
    pub rotate_degrees: i32,
}

impl InternalVideoOrientationConfig {
    /// 映射为 `videoflip` 的 `method` 属性字符串 / Maps to `videoflip` `method` property string.
    ///
    /// # 参数 / Parameters
    /// - 无 / None
    ///
    /// # 返回值 / Returns
    /// - GStreamer `videoflip` method 名称 / method name for GStreamer `videoflip`
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 纯函数 / Pure function
    ///
    /// # 平台 / Platform
    /// - 仅 playbin URI 管线支持 / playbin URI pipelines only
    pub fn method_name(&self) -> &'static str {
        match (
            self.flip_horizontal,
            self.flip_vertical,
            self.rotate_degrees,
        ) {
            (false, false, 0) => "none",
            (true, false, 0) => "horizontal-flip",
            (false, true, 0) => "vertical-flip",
            (false, false, 90) => "clockwise",
            (false, false, 180) => "rotate-180",
            (false, false, 270) => "counterclockwise",
            _ => "automatic",
        }
    }
}

/// 平台视频 sink 的宽高比缩放模式 / Aspect ratio scaling mode for the platform video sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum InternalAspectRatioMode {
    /// 保持宽高比适应（letterbox）/ Letterbox fit (default).
    #[default]
    Fit,
    /// 裁剪填充 / Crop to fill.
    Fill,
    /// 拉伸填满 / Stretch to fill.
    Stretch,
}

impl InternalAspectRatioMode {
    /// 将模式应用到视频 sink 元素属性 / Applies mode to video sink element properties.
    ///
    /// # 参数 / Parameters
    /// - `sink` — 平台视频 sink 元素 / platform video sink element
    ///
    /// # 返回值 / Returns
    /// - 无 / None
    ///
    /// # 错误 / Errors
    /// - 无（属性不存在时静默跳过）/ None (silently skips missing properties)
    ///
    /// # 线程 / Threading
    /// - 必须在 Gst 线程上调用 / Must run on Gst thread
    ///
    /// # 平台 / Platform
    /// - 依赖 sink 是否暴露 `force-aspect-ratio` / depends on sink exposing `force-aspect-ratio`
    pub fn apply_to_sink(&self, sink: &gst::Element) {
        match self {
            InternalAspectRatioMode::Fit => {
                if sink.find_property("force-aspect-ratio").is_some() {
                    sink.set_property("force-aspect-ratio", true);
                }
                if sink.find_property("pixel-aspect-ratio").is_some() {
                    sink.set_property("pixel-aspect-ratio", gst::Fraction::new(0, 1));
                }
            }
            InternalAspectRatioMode::Fill | InternalAspectRatioMode::Stretch => {
                if sink.find_property("force-aspect-ratio").is_some() {
                    sink.set_property("force-aspect-ratio", false);
                }
            }
        }
    }
}

/// 确保 playbin `video-filter` 上存在并配置 `videoflip` / Ensures `videoflip` on playbin `video-filter`.
///
/// # 参数 / Parameters
/// - `playbin` — playbin 元素 / playbin element
/// - `config` — 方向配置 / orientation config
///
/// # 返回值 / Returns
/// - 成功：`Ok(())`；`none` 时清除 filter / `Ok(())`; clears filter when `none`
///
/// # 错误 / Errors
/// - `videoflip` 元素创建失败 / videoflip element creation failure
///
/// # 线程 / Threading
/// - 必须在 Gst 线程上调用 / Must run on Gst thread
///
/// # 平台 / Platform
/// - 仅 URI playbin 管线（[`crate::playback::capabilities::PipelineCapabilities::PLAYBIN`]）
pub fn apply_orientation_to_playbin(
    playbin: &gst::Element,
    config: InternalVideoOrientationConfig,
) -> Result<()> {
    if config.method_name() == "none" {
        if playbin.find_property("video-filter").is_some() {
            playbin.set_property("video-filter", None::<gst::Element>);
        }
        return Ok(());
    }
    let flip = gst::ElementFactory::make("videoflip")
        .build()
        .map_err(|e| anyhow!("videoflip: {e}"))?;
    if flip.find_property("method").is_some() {
        flip.set_property_from_str("method", config.method_name());
    }
    playbin.set_property("video-filter", &flip);
    Ok(())
}
