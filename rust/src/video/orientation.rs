use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;

/// Video orientation applied via a `videoflip` element on playbin's video-filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct InternalVideoOrientationConfig {
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    /// Clockwise rotation in degrees: 0, 90, 180, or 270.
    pub rotate_degrees: i32,
}

impl InternalVideoOrientationConfig {
    pub fn method_name(&self) -> &'static str {
        match (self.flip_horizontal, self.flip_vertical, self.rotate_degrees) {
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

/// Aspect ratio scaling mode for the platform video sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum InternalAspectRatioMode {
    #[default]
    Fit,
    Fill,
    Stretch,
}

impl InternalAspectRatioMode {
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

/// Ensures a `videoflip` element exists on playbin's `video-filter` property.
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
