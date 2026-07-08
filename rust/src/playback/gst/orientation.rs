//! playbin 画面旋转与宽高比 / playbin rotation and aspect ratio.
//!
//! 通过 playbin `video-filter` 或 AppSrc 支路上的 `videoflip` 应用顺时针旋转，
//! 并通过 sink 属性控制 `force-aspect-ratio` 实现 Fit/Fill/Stretch 模式。
//!
//! Applies clockwise rotation via `videoflip` on playbin `video-filter` or the AppSrc
//! video branch, and controls `force-aspect-ratio` on the sink for Fit/Fill/Stretch modes.

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;

/// 校验旋转角度 / Validates clockwise rotation in degrees.
pub(crate) fn validate_rotate_degrees(degrees: i32) -> Result<i32> {
    match degrees {
        0 | 90 | 180 | 270 => Ok(degrees),
        _ => Err(anyhow!(
            "invalid rotate_degrees: {degrees}, expected 0, 90, 180, or 270"
        )),
    }
}

/// 映射为 `videoflip` 的 `method` 属性 / Maps degrees to GStreamer `videoflip` method name.
pub(crate) fn rotate_method(rotate_degrees: i32) -> &'static str {
    match rotate_degrees {
        90 => "clockwise",
        180 => "rotate-180",
        270 => "counterclockwise",
        _ => "none",
    }
}

fn update_videoflip_method(flip: &gst::Element, rotate_degrees: i32) -> Result<()> {
    if flip.find_property("method").is_some() {
        flip.set_property_from_str("method", rotate_method(rotate_degrees));
    }
    Ok(())
}

/// 在 AppSrc 支路 `videoflip` 上发送 flush，强制 caps 重协商 / Flushes AppSrc branch videoflip for caps renegotiation.
pub(crate) fn flush_videoflip_element(flip: &gst::Element) -> Result<()> {
    let Some(src_pad) = flip.static_pad("src") else {
        return Ok(());
    };
    if !src_pad.send_event(gst::event::FlushStart::new()) {
        return Ok(());
    }
    let _ = src_pad.send_event(gst::event::FlushStop::new(true));
    Ok(())
}

/// 创建初始 `videoflip` 元素（AppSrc 支路）/ Creates a `videoflip` for the AppSrc video branch.
pub(crate) fn make_videoflip_element() -> Result<gst::Element> {
    let flip = gst::ElementFactory::make("videoflip")
        .name("video-orientation")
        .build()
        .map_err(|e| anyhow!("videoflip: {e}"))?;
    update_videoflip_method(&flip, 0)?;
    Ok(flip)
}

/// 更新独立 `videoflip` 元素（AppSrc）/ Updates a standalone `videoflip` (AppSrc path).
pub(crate) fn apply_rotation_to_element(
    flip: &gst::Element,
    rotate_degrees: i32,
) -> Result<()> {
    validate_rotate_degrees(rotate_degrees)?;
    update_videoflip_method(flip, rotate_degrees)
}

fn set_playbin_video_filter(playbin: &gst::Element, flip: &gst::Element) {
    if playbin.find_property("video-filter").is_some() {
        playbin.set_property("video-filter", flip);
    }
}

/// 在 playbin `video-filter` 上应用旋转；复用已有 filter / Applies rotation on playbin `video-filter`, reusing when possible.
pub fn apply_rotation_to_playbin(
    playbin: &gst::Element,
    rotate_degrees: i32,
    cached: &mut Option<gst::Element>,
) -> Result<()> {
    validate_rotate_degrees(rotate_degrees)?;
    if rotate_method(rotate_degrees) == "none" {
        if playbin.find_property("video-filter").is_some() {
            playbin.set_property("video-filter", None::<gst::Element>);
        }
        *cached = None;
        return Ok(());
    }

    if let Some(ref flip) = cached {
        update_videoflip_method(flip, rotate_degrees)?;
        set_playbin_video_filter(playbin, flip);
        return Ok(());
    }

    if playbin.find_property("video-filter").is_some() {
        if let Some(existing) = playbin.property::<Option<gst::Element>>("video-filter") {
            update_videoflip_method(&existing, rotate_degrees)?;
            set_playbin_video_filter(playbin, &existing);
            *cached = Some(existing);
            return Ok(());
        }
    }

    let flip = gst::ElementFactory::make("videoflip")
        .build()
        .map_err(|e| anyhow!("videoflip: {e}"))?;
    update_videoflip_method(&flip, rotate_degrees)?;
    set_playbin_video_filter(playbin, &flip);
    *cached = Some(flip);
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::rotate_method;

    #[test]
    fn rotate_method_maps_four_corners() {
        assert_eq!(rotate_method(0), "none");
        assert_eq!(rotate_method(90), "clockwise");
        assert_eq!(rotate_method(180), "rotate-180");
        assert_eq!(rotate_method(270), "counterclockwise");
    }
}
