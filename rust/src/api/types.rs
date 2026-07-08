//! 跨 Dart/Rust 边界的 DTO 与事件类型 / DTOs and event types crossing the Dart–Rust boundary.
//!
//! 本模块定义 FRB 序列化的公开类型：播放状态、事件流 payload、轨道、元数据等。
//! 内部 GStreamer 类型通过 `From` 转换为此处 DTO 后推送给 Dart。
//!
//! Defines FRB-serializable public types: playback state, event stream payloads,
//! tracks, metadata, etc. Internal GStreamer types are converted via `From` before push to Dart.

use gstreamer as gst;

/// 上报给 Dart 的高层播放状态 / High-level playback state reported to Dart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    /// 空闲，尚未加载媒体 / Idle, no media loaded yet.
    Idle,
    /// 已就绪，可播放 / Ready, can start playback.
    Ready,
    /// 缓冲中 / Buffering.
    Buffering,
    /// 正在播放 / Playing.
    Playing,
    /// 已暂停 / Paused.
    Paused,
    /// 已停止（pipeline NULL）/ Stopped (pipeline NULL).
    Stopped,
    /// 播放到结尾 / Reached end of media.
    Completed,
    /// 发生错误 / Error state.
    Error,
}

/// 区分 [`PlayerEvent`] 中哪些字段有效 / Discriminates which fields of [`PlayerEvent`] are meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerEventKind {
    /// 总时长变更 / Duration changed.
    DurationChanged,
    /// 播放位置变更 / Position changed.
    PositionChanged,
    /// 视频尺寸变更 / Video dimensions changed.
    VideoSize,
    /// 播放状态变更 / Playback state changed.
    StateChanged,
    /// 缓冲进度 / Buffering progress.
    Buffering,
    /// 播放结束 / End of stream.
    Eos,
    /// 错误 / Error.
    Error,
    /// 可用轨道列表变更 / Available tracks changed.
    TracksChanged,
    /// 视频元数据变更 / Video metadata changed.
    MetadataChanged,
}

/// 当前媒体内的音轨、视频轨或字幕轨 / Audio, video, or subtitle stream inside the current media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTrack {
    /// GStreamer 流 ID / GStreamer stream id.
    pub id: u32,
    /// 轨道类型 / Track type.
    pub track_type: TrackType,
    /// ISO 639 语言码，可能为空 / ISO 639 language code, may be empty.
    pub language: String,
    /// 用户可见标签 / User-visible label.
    pub label: String,
    /// 是否当前选中 / Whether currently selected.
    pub selected: bool,
}

/// 媒体轨道类型 / Media track type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackType {
    /// 音频 / Audio.
    Audio,
    /// 视频 / Video.
    Video,
    /// 字幕 / Subtitle.
    Subtitle,
}

/// 解码后视频元数据，暴露给 Dart / Decoded video metadata surfaced to Dart.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoMetadata {
    /// 帧宽（像素）/ Frame width in pixels.
    pub width: i32,
    /// 帧高（像素）/ Frame height in pixels.
    pub height: i32,
    /// 帧率 / Frames per second.
    pub fps: f64,
    /// 像素宽高比分子 / Pixel aspect ratio numerator.
    pub pixel_aspect_width: i32,
    /// 像素宽高比分母 / Pixel aspect ratio denominator.
    pub pixel_aspect_height: i32,
    /// 显示宽高比分子 / Display aspect ratio numerator.
    pub display_aspect_width: i32,
    /// 显示宽高比分母 / Display aspect ratio denominator.
    pub display_aspect_height: i32,
    /// 是否为隔行扫描 / Whether interlaced.
    pub interlaced: bool,
    /// 色彩矩阵（字符串化枚举）/ Color matrix (stringified enum).
    pub color_matrix: String,
    /// 色彩范围（字符串化枚举）/ Color range (stringified enum).
    pub color_range: String,
    /// HDR 格式标识，如 `"HDR10"`，无 HDR 时为空 / HDR format tag, e.g. `"HDR10"`, empty if SDR.
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

/// 当前 pipeline 可用能力（playbin vs AppSrc）/ Features available on the active pipeline (playbin vs AppSrc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineCapabilitiesDto {
    /// 是否支持 seek / Whether seeking is supported.
    pub seek: bool,
    /// 是否支持多轨道选择 / Whether multi-track selection is supported.
    pub tracks: bool,
    /// 是否支持视频方向变换 / Whether video orientation transforms are supported.
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

/// 视频宽高比缩放模式 / Aspect ratio scaling mode for video display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AspectRatioMode {
    /// 保持比例， letterbox / Preserve aspect ratio (letterbox).
    #[default]
    Fit,
    /// 保持比例，裁剪填满 / Preserve aspect ratio (crop to fill).
    Fill,
    /// 拉伸填满 / Stretch to fill.
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

/// 统一加载 API 的媒体输入 / Media input for unified load API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaSourceDto {
    /// 网络或本地 URI，如 `https://...`、`file://...` / Network or local URI.
    Uri(String),
    /// Flutter 资源键，如 `assets/sample.mp4` / Flutter asset key.
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

/// 扁平事件结构，经 broadcast stream 推送给 Dart / Flat event struct pushed to Dart over a broadcast stream.
///
/// 根据 [`kind`](Self::kind) 判断哪些字段有效；未使用的字段保持默认值。
/// Use [`kind`](Self::kind) to determine meaningful fields; unused fields stay at defaults.
#[derive(Debug, Clone)]
pub struct PlayerEvent {
    /// 事件类型 / Event kind.
    pub kind: PlayerEventKind,
    /// 当前位置（毫秒）/ Current position in milliseconds.
    pub position_ms: i64,
    /// 总时长（毫秒）/ Total duration in milliseconds.
    pub duration_ms: i64,
    /// 视频宽（像素）/ Video width in pixels.
    pub width: i32,
    /// 视频高（像素）/ Video height in pixels.
    pub height: i32,
    /// 缓冲百分比 0–100 / Buffering percent 0–100.
    pub buffering_percent: i32,
    /// 当前播放状态 / Current playback state.
    pub state: PlayerState,
    /// 错误或附加消息 / Error or auxiliary message.
    pub message: String,
    /// 帧率 / Frames per second.
    pub fps: f64,
    /// 像素宽高比分子 / Pixel aspect ratio numerator.
    pub pixel_aspect_width: i32,
    /// 像素宽高比分母 / Pixel aspect ratio denominator.
    pub pixel_aspect_height: i32,
    /// 显示宽高比分子 / Display aspect ratio numerator.
    pub display_aspect_width: i32,
    /// 显示宽高比分母 / Display aspect ratio denominator.
    pub display_aspect_height: i32,
    /// 是否隔行 / Whether interlaced.
    pub interlaced: bool,
    /// 色彩矩阵 / Color matrix.
    pub color_matrix: String,
    /// 色彩范围 / Color range.
    pub color_range: String,
    /// HDR 格式 / HDR format.
    pub hdr_format: String,
    /// 是否可 seek / Whether media is seekable.
    pub is_seekable: bool,
}

impl PlayerEvent {
    /// 构造带默认字段的基础事件 / Build a base event with default field values.
    ///
    /// # 参数 / Parameters
    /// - `kind` — 事件类型 / event kind
    ///
    /// # 返回值 / Returns
    /// - 其余字段为默认值的 [`PlayerEvent`] / [`PlayerEvent`] with other fields at defaults
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

    /// 构造时长变更事件 / Build a duration-changed event.
    ///
    /// # 参数 / Parameters
    /// - `duration_ms` — 总时长（毫秒）/ total duration in ms
    pub(crate) fn duration(duration_ms: i64) -> Self {
        Self {
            duration_ms,
            ..Self::base(PlayerEventKind::DurationChanged)
        }
    }

    /// 构造位置变更事件 / Build a position-changed event.
    ///
    /// # 参数 / Parameters
    /// - `position_ms` — 当前位置（毫秒）/ current position in ms
    pub(crate) fn position(position_ms: i64) -> Self {
        Self {
            position_ms,
            ..Self::base(PlayerEventKind::PositionChanged)
        }
    }

    /// 构造视频尺寸变更事件 / Build a video-size-changed event.
    ///
    /// # 参数 / Parameters
    /// - `width` — 宽（像素）/ width in pixels
    /// - `height` — 高（像素）/ height in pixels
    pub(crate) fn video_size(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            ..Self::base(PlayerEventKind::VideoSize)
        }
    }

    /// 构造元数据变更事件 / Build a metadata-changed event.
    ///
    /// # 参数 / Parameters
    /// - `meta` — 内部解码元数据 / internal decoded metadata
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
        // seek 能力在加载时由 pipeline capabilities 决定，而非 video caps。
        // Seek capability is owned by pipeline capabilities at load time, not video caps.
        event.is_seekable = false;
        event
    }

    /// 构造状态变更事件 / Build a state-changed event.
    pub(crate) fn state(state: PlayerState) -> Self {
        Self {
            state,
            ..Self::base(PlayerEventKind::StateChanged)
        }
    }

    /// 构造缓冲事件 / Build a buffering event.
    ///
    /// # 参数 / Parameters
    /// - `buffering_percent` — 0–100
    pub(crate) fn buffering(buffering_percent: i32) -> Self {
        Self {
            buffering_percent,
            state: PlayerState::Buffering,
            ..Self::base(PlayerEventKind::Buffering)
        }
    }

    /// 构造 EOS 事件 / Build an end-of-stream event.
    pub(crate) fn eos() -> Self {
        Self::base(PlayerEventKind::Eos)
    }

    /// 构造错误事件 / Build an error event.
    ///
    /// # 参数 / Parameters
    /// - `message` — 错误描述 / error description
    pub(crate) fn error(message: String) -> Self {
        Self {
            message,
            state: PlayerState::Error,
            ..Self::base(PlayerEventKind::Error)
        }
    }

    /// 构造轨道列表变更事件 / Build a tracks-changed event.
    pub(crate) fn tracks_changed() -> Self {
        Self::base(PlayerEventKind::TracksChanged)
    }
}

/// 将 GStreamer `State` 映射为 Dart 侧 [`PlayerState`] / Map GStreamer `State` to Dart [`PlayerState`].
///
/// # 参数 / Parameters
/// - `state` — GStreamer pipeline 状态 / GStreamer pipeline state
///
/// # 返回值 / Returns
/// - 对应的 [`PlayerState`] / corresponding [`PlayerState`]
pub(crate) fn map_state(state: gst::State) -> PlayerState {
    match state {
        gst::State::Null => PlayerState::Stopped,
        gst::State::Ready => PlayerState::Ready,
        gst::State::Paused => PlayerState::Paused,
        gst::State::Playing => PlayerState::Playing,
        _ => PlayerState::Idle,
    }
}
