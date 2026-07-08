use gstreamer as gst;
use gstreamer_video as gst_video;

/// Internal decoded video metadata (converted to [`crate::player_events::VideoMetadata`] for Dart).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct InternalVideoMetadata {
    pub width: i32,
    pub height: i32,
    pub fps: f64,
    pub pixel_aspect_width: i32,
    pub pixel_aspect_height: i32,
    pub display_aspect_width: i32,
    pub display_aspect_height: i32,
    pub interlaced: bool,
    pub color_matrix: String,
    pub color_range: String,
    pub hdr_format: String,
}

impl InternalVideoMetadata {
    pub fn from_video_info(info: &gst_video::VideoInfo) -> Self {
        Self::from_video_info_and_caps(info, info.to_caps().ok().as_ref().map(|c| c.as_ref()))
    }

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

fn detect_hdr_format(caps: &gst::CapsRef) -> String {
    if let Ok(info) = gst_video::VideoInfo::from_caps(caps) {
        return detect_hdr_from_colorimetry(&info.colorimetry());
    }
    String::new()
}
