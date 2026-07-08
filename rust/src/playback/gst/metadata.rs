//! 解码元数据提取 / Decoded video metadata extraction.
//!
//! 从 GStreamer [`gst_video::VideoInfo`] 与 caps 提取宽高、帧率、PAR/DAR、色域与 HDR 信息，
//! 供 [`crate::playback::sink::attach_video_probe`] 缓存并经 FRB 转为 Dart [`VideoMetadata`]。
//!
//! Extracts width, height, fps, PAR/DAR, colorimetry, and HDR from
//! [`gst_video::VideoInfo`] and caps; cached by [`crate::playback::sink::attach_video_probe`]
//! and converted to Dart [`VideoMetadata`] via FRB.

use gstreamer as gst;
use gstreamer_video as gst_video;

/// 内部解码视频元数据（经 FRB 转为 [`crate::api::types::VideoMetadata`]）/ Internal decoded video metadata (converted to Dart via FRB).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct InternalVideoMetadata {
    /// 像素宽度 / Pixel width.
    pub width: i32,
    /// 像素高度 / Pixel height.
    pub height: i32,
    /// 帧率（fps）/ Frame rate in fps.
    pub fps: f64,
    /// 像素宽高比分子 / Pixel aspect ratio numerator.
    pub pixel_aspect_width: i32,
    /// 像素宽高比分母 / Pixel aspect ratio denominator.
    pub pixel_aspect_height: i32,
    /// 显示宽高比分子 / Display aspect ratio numerator.
    pub display_aspect_width: i32,
    /// 显示宽高比分母 / Display aspect ratio denominator.
    pub display_aspect_height: i32,
    /// 是否隔行扫描 / Whether interlaced.
    pub interlaced: bool,
    /// 色彩矩阵（Debug 字符串）/ Color matrix (debug string).
    pub color_matrix: String,
    /// 色彩范围（Debug 字符串）/ Color range (debug string).
    pub color_range: String,
    /// HDR 格式标识（如 HDR10、HLG）/ HDR format label (e.g. HDR10, HLG).
    pub hdr_format: String,
}

impl InternalVideoMetadata {
    /// 从 [`VideoInfo`] 构建元数据 / Builds metadata from [`VideoInfo`].
    ///
    /// # 参数 / Parameters
    /// - `info` — 协商后的视频信息 / negotiated video info
    ///
    /// # 返回值 / Returns
    /// - 填充的 [`InternalVideoMetadata`] / populated metadata
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 纯函数 / Pure function
    ///
    /// # 平台 / Platform
    /// - 与平台无关 / Platform-independent
    pub fn from_video_info(info: &gst_video::VideoInfo) -> Self {
        Self::from_video_info_and_caps(info, info.to_caps().ok().as_ref().map(|c| c.as_ref()))
    }

    /// 从 [`VideoInfo`] 与可选 caps 构建元数据（含 HDR 检测）/ Builds metadata with optional caps for HDR detection.
    ///
    /// # 参数 / Parameters
    /// - `info` — 视频信息 / video info
    /// - `caps` — 可选 caps 引用（增强 HDR 检测）/ optional caps for HDR detection
    ///
    /// # 返回值 / Returns
    /// - 填充的 [`InternalVideoMetadata`] / populated metadata
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 通常在 pad probe 回调中调用 / typically called from pad probe callback
    ///
    /// # 平台 / Platform
    /// - 与平台无关 / Platform-independent
    pub fn from_video_info_and_caps(
        info: &gst_video::VideoInfo,
        caps: Option<&gst::CapsRef>,
    ) -> Self {
        let fps = if info.fps().numer() > 0 {
            info.fps().numer() as f64 / info.fps().denom().max(1) as f64
        } else {
            0.0
        };
        let par = info.par();
        let w = info.width() as i32;
        let h = info.height().max(1) as i32;
        let (display_aspect_width, display_aspect_height) = if par.numer() > 0 && par.denom() > 0 {
            (
                (w as i64 * par.numer() as i64) as i32,
                (h as i64 * par.denom() as i64) as i32,
            )
        } else {
            (w, h)
        };
        let colorimetry = info.colorimetry();
        let hdr_format = caps
            .map(detect_hdr_format)
            .unwrap_or_else(|| detect_hdr_from_colorimetry(&colorimetry));
        Self {
            width: w,
            height: h,
            fps,
            pixel_aspect_width: par.numer() as i32,
            pixel_aspect_height: par.denom().max(1) as i32,
            display_aspect_width,
            display_aspect_height,
            interlaced: info.is_interlaced(),
            color_matrix: format!("{:?}", colorimetry.matrix()),
            color_range: format!("{:?}", colorimetry.range()),
            hdr_format,
        }
    }

    /// 计算显示宽高比（浮点）/ Computes display aspect ratio as float.
    ///
    /// # 参数 / Parameters
    /// - 无 / None
    ///
    /// # 返回值 / Returns
    /// - DAR 浮点值；缺省时回退 16:9 / DAR float; falls back to 16:9
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 纯函数 / Pure function
    ///
    /// # 平台 / Platform
    /// - 与平台无关 / Platform-independent
    pub fn display_aspect_ratio(&self) -> f64 {
        if self.display_aspect_height > 0 {
            self.display_aspect_width as f64 / self.display_aspect_height as f64
        } else if self.height > 0 {
            self.width as f64 / self.height as f64
        } else {
            16.0 / 9.0
        }
    }
}

/// 从色彩传递函数推断 HDR 格式 / Infers HDR format from color transfer function.
fn detect_hdr_from_colorimetry(colorimetry: &gst_video::VideoColorimetry) -> String {
    match colorimetry.transfer() {
        gst_video::VideoTransferFunction::Bt202012 => "HDR10".to_string(),
        other => {
            let name = format!("{other:?}");
            if name.contains("Smpte2084") || name.contains("Bt2020") {
                "HDR10".to_string()
            } else if name.contains("Arib") || name.contains("B67") {
                "HLG".to_string()
            } else {
                String::new()
            }
        }
    }
}

/// 从 caps 检测 HDR 格式 / Detects HDR format from caps.
fn detect_hdr_format(caps: &gst::CapsRef) -> String {
    if let Ok(info) = gst_video::VideoInfo::from_caps(caps) {
        return detect_hdr_from_colorimetry(&info.colorimetry());
    }
    String::new()
}
