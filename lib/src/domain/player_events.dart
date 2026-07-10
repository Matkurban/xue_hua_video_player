/// Hand-owned player domain types (no flutter_rust_bridge).
library;

// ignore_for_file: camel_case_types

/// 视频宽高比缩放模式 / Aspect ratio scaling mode for video display.
enum AspectRatioMode {
  /// 保持比例， letterbox / Preserve aspect ratio (letterbox).
  fit,

  /// 保持比例，裁剪填满 / Preserve aspect ratio (crop to fill).
  fill,

  /// 拉伸填满 / Stretch to fill.
  stretch,
}

/// 网络或本地 URI / Flutter asset 媒体源。
sealed class MediaSourceDto {
  const MediaSourceDto();

  /// 网络或本地 URI，如 `https://...`、`file://...`
  const factory MediaSourceDto.uri(String field0) = MediaSourceDto_Uri;

  /// Flutter 资源键，如 `assets/sample.mp4`
  const factory MediaSourceDto.flutterAsset(String field0) =
      MediaSourceDto_FlutterAsset;
}

final class MediaSourceDto_Uri extends MediaSourceDto {
  const MediaSourceDto_Uri(this.field0);
  final String field0;
}

final class MediaSourceDto_FlutterAsset extends MediaSourceDto {
  const MediaSourceDto_FlutterAsset(this.field0);
  final String field0;
}

/// 媒体轨道类型。
enum TrackType { audio, video, subtitle }

/// 当前媒体内的音轨、视频轨或字幕轨。
class MediaTrack {
  const MediaTrack({
    required this.id,
    required this.trackType,
    required this.language,
    required this.label,
    required this.selected,
  });

  final int id;
  final TrackType trackType;
  final String language;
  final String label;
  final bool selected;

  @override
  int get hashCode => Object.hash(id, trackType, language, label, selected);

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is MediaTrack &&
          id == other.id &&
          trackType == other.trackType &&
          language == other.language &&
          label == other.label &&
          selected == other.selected;
}

/// 当前 pipeline 可用能力。
class PipelineCapabilitiesDto {
  const PipelineCapabilitiesDto({
    required this.seek,
    required this.tracks,
    required this.orientation,
  });

  final bool seek;
  final bool tracks;
  final bool orientation;

  @override
  int get hashCode => Object.hash(seek, tracks, orientation);

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PipelineCapabilitiesDto &&
          seek == other.seek &&
          tracks == other.tracks &&
          orientation == other.orientation;
}

/// 高层播放状态。
enum PlayerState {
  idle,
  ready,
  buffering,
  playing,
  paused,
  stopped,
  completed,
  error,
}

/// 事件类型。
enum PlayerEventKind {
  durationChanged,
  positionChanged,
  videoSize,
  stateChanged,
  buffering,
  eos,
  error,
  tracksChanged,
  metadataChanged,
}

/// 扁平事件结构。
class PlayerEvent {
  const PlayerEvent({
    required this.kind,
    required this.positionMs,
    required this.durationMs,
    required this.width,
    required this.height,
    required this.bufferingPercent,
    required this.state,
    required this.message,
    required this.fps,
    required this.pixelAspectWidth,
    required this.pixelAspectHeight,
    required this.displayAspectWidth,
    required this.displayAspectHeight,
    required this.interlaced,
    required this.colorMatrix,
    required this.colorRange,
    required this.hdrFormat,
    required this.isSeekable,
  });

  final PlayerEventKind kind;
  final int positionMs;
  final int durationMs;
  final int width;
  final int height;
  final int bufferingPercent;
  final PlayerState state;
  final String message;
  final double fps;
  final int pixelAspectWidth;
  final int pixelAspectHeight;
  final int displayAspectWidth;
  final int displayAspectHeight;
  final bool interlaced;
  final String colorMatrix;
  final String colorRange;
  final String hdrFormat;
  final bool isSeekable;

  @override
  int get hashCode => Object.hashAll([
    kind,
    positionMs,
    durationMs,
    width,
    height,
    bufferingPercent,
    state,
    message,
    fps,
    pixelAspectWidth,
    pixelAspectHeight,
    displayAspectWidth,
    displayAspectHeight,
    interlaced,
    colorMatrix,
    colorRange,
    hdrFormat,
    isSeekable,
  ]);

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PlayerEvent &&
          kind == other.kind &&
          positionMs == other.positionMs &&
          durationMs == other.durationMs &&
          width == other.width &&
          height == other.height &&
          bufferingPercent == other.bufferingPercent &&
          state == other.state &&
          message == other.message &&
          fps == other.fps &&
          pixelAspectWidth == other.pixelAspectWidth &&
          pixelAspectHeight == other.pixelAspectHeight &&
          displayAspectWidth == other.displayAspectWidth &&
          displayAspectHeight == other.displayAspectHeight &&
          interlaced == other.interlaced &&
          colorMatrix == other.colorMatrix &&
          colorRange == other.colorRange &&
          hdrFormat == other.hdrFormat &&
          isSeekable == other.isSeekable;
}

/// 解码后视频元数据。
class VideoMetadata {
  const VideoMetadata({
    required this.width,
    required this.height,
    required this.fps,
    required this.pixelAspectWidth,
    required this.pixelAspectHeight,
    required this.displayAspectWidth,
    required this.displayAspectHeight,
    required this.interlaced,
    required this.colorMatrix,
    required this.colorRange,
    required this.hdrFormat,
  });

  final int width;
  final int height;
  final double fps;
  final int pixelAspectWidth;
  final int pixelAspectHeight;
  final int displayAspectWidth;
  final int displayAspectHeight;
  final bool interlaced;
  final String colorMatrix;
  final String colorRange;
  final String hdrFormat;

  @override
  int get hashCode => Object.hashAll([
    width,
    height,
    fps,
    pixelAspectWidth,
    pixelAspectHeight,
    displayAspectWidth,
    displayAspectHeight,
    interlaced,
    colorMatrix,
    colorRange,
    hdrFormat,
  ]);

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is VideoMetadata &&
          width == other.width &&
          height == other.height &&
          fps == other.fps &&
          pixelAspectWidth == other.pixelAspectWidth &&
          pixelAspectHeight == other.pixelAspectHeight &&
          displayAspectWidth == other.displayAspectWidth &&
          displayAspectHeight == other.displayAspectHeight &&
          interlaced == other.interlaced &&
          colorMatrix == other.colorMatrix &&
          colorRange == other.colorRange &&
          hdrFormat == other.hdrFormat;
}
