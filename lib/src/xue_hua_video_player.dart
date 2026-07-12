import 'dart:io';

import 'package:flutter/services.dart';

import 'ffi/xhvp_library.dart';
import 'media/frame_image.dart';
import 'media/media_source_resolver.dart';
import 'model/video_source.dart';
import 'enum/video_source_type.dart';
import 'domain/player_events.dart';
import 'player/ffi_native_worker.dart';

/// 插件入口：加载原生库并初始化 GStreamer 运行时。
///
/// 在创建 [XueHuaPlayerController] 之前调用 [initialize] 一次：
///
/// ```dart
/// Future<void> main() async {
///   WidgetsFlutterBinding.ensureInitialized();
///   await XueHuaVideoPlayer.initialize();
///   runApp(const MyApp());
/// }
/// ```
class XueHuaVideoPlayer {
  const XueHuaVideoPlayer._();

  static bool _initialized = false;

  /// 加载原生库并初始化 C 播放器运行时。幂等；热重启后可重复调用。
  static Future<void> initialize() async {
    if (_initialized) return;
    final code = XhvpLibrary.instance.init();
    if (code != 0) {
      throw StateError('xhvp_init failed with code $code');
    }
    _initialized = true;
  }

  /// 从 [source] 抽取一帧封面，返回 PNG 字节。
  ///
  /// [at] 为 null 时由 native 自动选点（约 5% 时长或 1 秒）。
  /// [maxWidth] 限制输出宽度（默认 320）。
  ///
  /// 须先调用 [initialize]。不占用播放中的 controller。
  static Future<Uint8List> captureThumbnail(
    VideoSource source, {
    Duration? at,
    int maxWidth = 320,
  }) async {
    await initialize();
    final resolved = await _resolveThumbnailUri(source);
    try {
      final worker = await FfiNativeWorker.ensureStarted();
      final map = await worker.run<Map<String, Object?>>(
        FfiThumbnailCaptureRequest(
          uri: resolved.uri,
          positionMs: at?.inMilliseconds ?? -1,
          maxWidth: maxWidth,
        ),
      );
      return CapturedBgraFrame.fromMap(map).toPng();
    } finally {
      final temp = resolved.tempFile;
      if (temp != null) {
        try {
          if (await temp.exists()) {
            await temp.delete();
          }
        } catch (_) {
          // Best-effort cleanup of asset staging file.
        }
      }
    }
  }

  static Future<({String uri, File? tempFile})> _resolveThumbnailUri(
    VideoSource source,
  ) async {
    switch (source.type) {
      case VideoSourceType.network:
      case VideoSourceType.file:
        final dto = const MediaSourceResolver().resolve(source);
        final uri = switch (dto) {
          MediaSourceDto_Uri(:final field0) => field0,
          MediaSourceDto_FlutterAsset() =>
            throw StateError('unexpected asset dto'),
        };
        return (uri: uri, tempFile: null);
      case VideoSourceType.asset:
        final key = source.uri.trim();
        final data = await rootBundle.load(key);
        final bytes = data.buffer.asUint8List(
          data.offsetInBytes,
          data.lengthInBytes,
        );
        if (bytes.isEmpty) {
          throw StateError('captureThumbnail: empty asset $key');
        }
        final safe = key.replaceAll(RegExp(r'[^A-Za-z0-9._-]'), '_');
        final file = File(
          '${Directory.systemTemp.path}${Platform.pathSeparator}'
          'xhvp_thumb_${DateTime.now().microsecondsSinceEpoch}_$safe',
        );
        await file.writeAsBytes(bytes, flush: true);
        return (uri: Uri.file(file.path).toString(), tempFile: file);
    }
  }
}
