/// Where a piece of media comes from.
enum VideoSourceType {
  /// A remote URL (`http(s)://`, `rtsp://`, ...).
  network,

  /// A local filesystem path or `file://` URI.
  file,

  /// A Flutter asset key declared in `pubspec.yaml`.
  asset,
}
