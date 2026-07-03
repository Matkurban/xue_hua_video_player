/// Where a piece of media comes from.
enum VideoSourceType {
  /// A remote URL (`http(s)://`, `rtsp://`, ...).
  network,

  /// A local filesystem path or `file://` URI.
  file,

  /// A Flutter asset key declared in `pubspec.yaml`.
  asset,
}

/// Describes the media a [XueHuaPlayerController] should load.
///
/// A [VideoSource] carries just two things: the [uri] (address / path / asset
/// key) and its [type]. Use one of the named constructors:
///
/// ```dart
/// controller.open(VideoSource.network('https://.../video.mp4'));
/// controller.open(VideoSource.file('/path/to/video.mp4'));
/// controller.open(const VideoSource.asset('assets/sample.mp4'));
/// ```
class VideoSource {
  /// Media served from a remote [url].
  const VideoSource.network(this.uri) : type = VideoSourceType.network;

  /// Media read from a local filesystem [path] (or a `file://` URI).
  const VideoSource.file(this.uri) : type = VideoSourceType.file;

  /// Media bundled as a Flutter asset, identified by its [assetKey]
  /// (e.g. `assets/sample.mp4`).
  const VideoSource.asset(this.uri) : type = VideoSourceType.asset;

  /// The address, local path, or asset key, depending on [type].
  final String uri;

  /// How [uri] should be interpreted when loading.
  final VideoSourceType type;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is VideoSource &&
          runtimeType == other.runtimeType &&
          uri == other.uri &&
          type == other.type;

  @override
  int get hashCode => Object.hash(uri, type);

  @override
  String toString() => 'VideoSource(${type.name}, $uri)';
}
