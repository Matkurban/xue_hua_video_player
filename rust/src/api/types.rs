use gstreamer as gst;

/// High-level playback state reported to Dart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Idle,
    Ready,
    Buffering,
    Playing,
    Paused,
    Stopped,
    Completed,
    Error,
}

/// Discriminates which fields of [`PlayerEvent`] are meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerEventKind {
    DurationChanged,
    PositionChanged,
    VideoSize,
    StateChanged,
    Buffering,
    Eos,
    Error,
    TracksChanged,
    MetadataChanged,
}

/// Audio, video, or subtitle stream inside the current media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTrack {
    pub id: u32,
    pub track_type: TrackType,
    pub language: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackType {
    Audio,
    Video,
    Subtitle,
}

/// Decoded video metadata surfaced to Dart.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoMetadata {
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

impl Default for VideoMetadata {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            fps: 0.0,
            pixel_aspect_width: 1,
            pixel_aspect_height: 1,
            display_aspect_width: 16,
            display_aspect_height: 9,
            interlaced: false,
            color_matrix: String::new(),
            color_range: String::new(),
            hdr_format: String::new(),
        }
    }
}

impl From<crate::playback::gst::InternalVideoMetadata> for VideoMetadata {
    fn from(m: crate::playback::gst::InternalVideoMetadata) -> Self {
        Self {
            width: m.width,
            height: m.height,
            fps: m.fps,
            pixel_aspect_width: m.pixel_aspect_width,
            pixel_aspect_height: m.pixel_aspect_height,
            display_aspect_width: m.display_aspect_width,
            display_aspect_height: m.display_aspect_height,
            interlaced: m.interlaced,
            color_matrix: m.color_matrix,
            color_range: m.color_range,
            hdr_format: m.hdr_format,
        }
    }
}

/// Features available on the active pipeline (playbin vs AppSrc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineCapabilitiesDto {
    pub seek: bool,
    pub tracks: bool,
    pub orientation: bool,
}

impl From<crate::playback::capabilities::PipelineCapabilities> for PipelineCapabilitiesDto {
    fn from(caps: crate::playback::capabilities::PipelineCapabilities) -> Self {
        Self {
            seek: caps.seek,
            tracks: caps.tracks,
            orientation: caps.orientation,
        }
    }
}

/// Video flip/rotate configuration for Dart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VideoOrientationConfig {
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub rotate_degrees: i32,
}

impl From<VideoOrientationConfig> for crate::playback::gst::InternalVideoOrientationConfig {
    fn from(c: VideoOrientationConfig) -> Self {
        Self {
            flip_horizontal: c.flip_horizontal,
            flip_vertical: c.flip_vertical,
            rotate_degrees: c.rotate_degrees,
        }
    }
}

impl From<crate::playback::gst::InternalVideoOrientationConfig> for VideoOrientationConfig {
    fn from(c: crate::playback::gst::InternalVideoOrientationConfig) -> Self {
        Self {
            flip_horizontal: c.flip_horizontal,
            flip_vertical: c.flip_vertical,
            rotate_degrees: c.rotate_degrees,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AspectRatioMode {
    #[default]
    Fit,
    Fill,
    Stretch,
}

impl From<AspectRatioMode> for crate::playback::gst::InternalAspectRatioMode {
    fn from(m: AspectRatioMode) -> Self {
        match m {
            AspectRatioMode::Fit => Self::Fit,
            AspectRatioMode::Fill => Self::Fill,
            AspectRatioMode::Stretch => Self::Stretch,
        }
    }
}

/// Media input for unified load API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaSourceDto {
    Uri(String),
    FlutterAsset(String),
}

impl From<MediaSourceDto> for crate::media::MediaSource {
    fn from(dto: MediaSourceDto) -> Self {
        match dto {
            MediaSourceDto::Uri(u) => Self::Uri(u),
            MediaSourceDto::FlutterAsset(k) => Self::FlutterAsset(k),
        }
    }
}

/// A flat event struct pushed to Dart over a broadcast stream.
#[derive(Debug, Clone)]
pub struct PlayerEvent {
    pub kind: PlayerEventKind,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub width: i32,
    pub height: i32,
    pub buffering_percent: i32,
    pub state: PlayerState,
    pub message: String,
    pub fps: f64,
    pub pixel_aspect_width: i32,
    pub pixel_aspect_height: i32,
    pub display_aspect_width: i32,
    pub display_aspect_height: i32,
    pub interlaced: bool,
    pub color_matrix: String,
    pub color_range: String,
    pub hdr_format: String,
    pub is_seekable: bool,
}

impl PlayerEvent {
    pub(crate) fn base(kind: PlayerEventKind) -> Self {
        Self {
            kind,
            position_ms: 0,
            duration_ms: 0,
            width: 0,
            height: 0,
            buffering_percent: 0,
            state: PlayerState::Idle,
            message: String::new(),
            fps: 0.0,
            pixel_aspect_width: 1,
            pixel_aspect_height: 1,
            display_aspect_width: 16,
            display_aspect_height: 9,
            interlaced: false,
            color_matrix: String::new(),
            color_range: String::new(),
            hdr_format: String::new(),
            is_seekable: true,
        }
    }

    pub(crate) fn duration(duration_ms: i64) -> Self {
        Self {
            duration_ms,
            ..Self::base(PlayerEventKind::DurationChanged)
        }
    }

    pub(crate) fn position(position_ms: i64) -> Self {
        Self {
            position_ms,
            ..Self::base(PlayerEventKind::PositionChanged)
        }
    }

    pub(crate) fn video_size(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            ..Self::base(PlayerEventKind::VideoSize)
        }
    }

    pub(crate) fn metadata(meta: crate::playback::gst::InternalVideoMetadata) -> Self {
        let mut event = Self {
            kind: PlayerEventKind::MetadataChanged,
            width: meta.width,
            height: meta.height,
            fps: meta.fps,
            pixel_aspect_width: meta.pixel_aspect_width,
            pixel_aspect_height: meta.pixel_aspect_height,
            display_aspect_width: meta.display_aspect_width,
            display_aspect_height: meta.display_aspect_height,
            interlaced: meta.interlaced,
            color_matrix: meta.color_matrix,
            color_range: meta.color_range,
            hdr_format: meta.hdr_format,
            ..Self::base(PlayerEventKind::MetadataChanged)
        };
        // Seek capability is owned by pipeline capabilities at load time, not video caps.
        event.is_seekable = false;
        event
    }

    pub(crate) fn state(state: PlayerState) -> Self {
        Self {
            state,
            ..Self::base(PlayerEventKind::StateChanged)
        }
    }

    pub(crate) fn buffering(buffering_percent: i32) -> Self {
        Self {
            buffering_percent,
            state: PlayerState::Buffering,
            ..Self::base(PlayerEventKind::Buffering)
        }
    }

    pub(crate) fn eos() -> Self {
        Self::base(PlayerEventKind::Eos)
    }

    pub(crate) fn error(message: String) -> Self {
        Self {
            message,
            state: PlayerState::Error,
            ..Self::base(PlayerEventKind::Error)
        }
    }

    pub(crate) fn tracks_changed() -> Self {
        Self::base(PlayerEventKind::TracksChanged)
    }
}

pub(crate) fn map_state(state: gst::State) -> PlayerState {
    match state {
        gst::State::Null => PlayerState::Stopped,
        gst::State::Ready => PlayerState::Ready,
        gst::State::Paused => PlayerState::Paused,
        gst::State::Playing => PlayerState::Playing,
        _ => PlayerState::Idle,
    }
}
