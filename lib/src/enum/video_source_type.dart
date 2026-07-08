/// 媒体来源类型 / Where a piece of media comes from.
///
/// 由 [MediaSourceResolver] 映射为 GStreamer 可消费的 URI 或 Flutter asset 描述符。
/// Mapped by [MediaSourceResolver] into a GStreamer URI or Flutter asset descriptor.
enum VideoSourceType {
  /// 远程 URL（`http(s)://`、`rtsp://` 等）/ Remote URL.
  network,

  /// 本地文件系统路径或 `file://` URI / Local filesystem path or `file://` URI.
  file,

  /// 在 `pubspec.yaml` 中声明的 Flutter 资源 / Flutter asset key in `pubspec.yaml`.
  asset,
}
