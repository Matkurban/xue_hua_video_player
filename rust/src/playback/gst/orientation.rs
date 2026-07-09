//! playbin 画面旋转与宽高比 / playbin rotation and aspect ratio.
//!
//! 通过 video-sink bin 或 AppSrc 支路上的 `videoflip`（非 Android）或 `gltransformation`（Android）应用顺时针旋转，
//! 并通过 sink 属性控制 `force-aspect-ratio` 实现 Fit/Fill/Stretch 模式。
//!
//! Applies clockwise rotation via in-pipeline `videoflip` on the playbin video-sink bin or
//! the AppSrc video branch, and controls `force-aspect-ratio` on the sink for Fit/Fill/Stretch.

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video::VideoOrientationMethod;

/// 校验旋转角度 / Validates clockwise rotation in degrees.
pub(crate) fn validate_rotate_degrees(degrees: i32) -> Result<i32> {
    match degrees {
        0 | 90 | 180 | 270 => Ok(degrees),
        _ => Err(anyhow!(
            "invalid rotate_degrees: {degrees}, expected 0, 90, 180, or 270"
        )),
    }
}

/// 映射为 `videoflip` 的 `method` 属性（旧 API 回退）/ Maps degrees to legacy `videoflip` method name.
pub(crate) fn rotate_method(rotate_degrees: i32) -> &'static str {
    match rotate_degrees {
        90 => "clockwise",
        180 => "rotate-180",
        270 => "counterclockwise",
        _ => "none",
    }
}

/// 映射为 `videoflip` 的 `video-direction` 属性 / Maps degrees to `VideoOrientationMethod`.
pub(crate) fn rotate_video_direction(rotate_degrees: i32) -> VideoOrientationMethod {
    match rotate_degrees {
        90 => VideoOrientationMethod::_90r,
        180 => VideoOrientationMethod::_180,
        270 => VideoOrientationMethod::_90l,
        _ => VideoOrientationMethod::Identity,
    }
}

fn current_videoflip_degrees(flip: &gst::Element) -> Option<i32> {
    if flip.find_property("video-direction").is_some() {
        let current: VideoOrientationMethod = flip.property("video-direction");
        return Some(match current {
            VideoOrientationMethod::_90r => 90,
            VideoOrientationMethod::_180 => 180,
            VideoOrientationMethod::_90l => 270,
            _ => 0,
        });
    }
    None
}

fn update_videoflip_direction(flip: &gst::Element, rotate_degrees: i32) -> Result<()> {
    if current_videoflip_degrees(flip) == Some(rotate_degrees) {
        return Ok(());
    }
    if flip.find_property("video-direction").is_some() {
        flip.set_property("video-direction", rotate_video_direction(rotate_degrees));
    } else if flip.find_property("method").is_some() {
        flip.set_property_from_str("method", rotate_method(rotate_degrees));
    }
    Ok(())
}

/// 创建 Android GL 旋转元素 `gltransformation` / Creates Android `gltransformation` for GL video branches.
#[cfg(target_os = "android")]
pub(crate) fn make_android_orientation_element() -> Result<gst::Element> {
    let element = gst::ElementFactory::make("gltransformation")
        .name("video-orientation")
        .build()
        .map_err(|e| anyhow!("gltransformation: {e}"))?;
    apply_rotation_to_gltransformation(&element, 0)?;
    Ok(element)
}

/// 平台旋转元素：`gltransformation`（Android）或 `videoflip`（其他）/ Platform orientation element.
pub(crate) fn make_orientation_element() -> Result<gst::Element> {
    #[cfg(target_os = "android")]
    {
        return make_android_orientation_element();
    }
    #[cfg(not(target_os = "android"))]
    {
        make_videoflip_element()
    }
}

/// `gltransformation` 的 `rotation-z`（度）；顺时针与 `videoflip` 一致。
#[cfg(target_os = "android")]
fn gltransformation_rotation_z(rotate_degrees: i32) -> f32 {
    match rotate_degrees {
        90 => -90.0_f32,
        180 => 180.0_f32,
        270 => -270.0_f32,
        _ => 0.0_f32,
    }
}

#[cfg(target_os = "android")]
fn current_gltransformation_degrees(element: &gst::Element) -> Option<i32> {
    if !element.find_property("rotation-z").is_some() {
        return None;
    }
    let z: f32 = element.property("rotation-z");
    Some(match z as i32 {
        -90 => 90,
        180 | -180 => 180,
        90 | -270 => 270,
        _ => 0,
    })
}

/// 更新 `gltransformation` 顺时针旋转角度 / Updates clockwise rotation on `gltransformation`.
#[cfg(target_os = "android")]
pub(crate) fn apply_rotation_to_gltransformation(
    element: &gst::Element,
    rotate_degrees: i32,
) -> Result<()> {
    validate_rotate_degrees(rotate_degrees)?;
    if current_gltransformation_degrees(element) == Some(rotate_degrees) {
        return Ok(());
    }
    if element.find_property("rotation-z").is_some() {
        element.set_property("rotation-z", gltransformation_rotation_z(rotate_degrees));
    }
    Ok(())
}

/// 创建初始 `videoflip` 元素 / Creates a `videoflip` for video sink bin or AppSrc branch.
pub(crate) fn make_videoflip_element() -> Result<gst::Element> {
    let flip = gst::ElementFactory::make("videoflip")
        .name("video-orientation")
        .build()
        .map_err(|e| anyhow!("videoflip: {e}"))?;
    update_videoflip_direction(&flip, 0)?;
    Ok(flip)
}

/// 更新已链接管线中的旋转元素（`videoflip` 或 `gltransformation`）/ Updates in-pipeline orientation element.
pub(crate) fn apply_rotation_to_element(flip: &gst::Element, rotate_degrees: i32) -> Result<()> {
    validate_rotate_degrees(rotate_degrees)?;
    #[cfg(target_os = "android")]
    if flip.find_property("rotation-z").is_some() {
        return apply_rotation_to_gltransformation(flip, rotate_degrees);
    }
    update_videoflip_direction(flip, rotate_degrees)
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
    use super::{rotate_method, rotate_video_direction};
    use gstreamer_video::VideoOrientationMethod;

    #[test]
    fn rotate_method_maps_four_corners() {
        assert_eq!(rotate_method(0), "none");
        assert_eq!(rotate_method(90), "clockwise");
        assert_eq!(rotate_method(180), "rotate-180");
        assert_eq!(rotate_method(270), "counterclockwise");
    }

    #[test]
    fn rotate_video_direction_maps_four_corners() {
        assert_eq!(rotate_video_direction(0), VideoOrientationMethod::Identity);
        assert_eq!(rotate_video_direction(90), VideoOrientationMethod::_90r);
        assert_eq!(rotate_video_direction(180), VideoOrientationMethod::_180);
        assert_eq!(rotate_video_direction(270), VideoOrientationMethod::_90l);
    }
}
