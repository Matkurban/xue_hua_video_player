import 'dart:async';
import 'dart:ffi';
import 'dart:io';

import 'package:flutter/services.dart';

import 'ffi/init_timing.dart';
import 'ffi/xhvp_bindings.dart';
import 'ffi/xhvp_library.dart';
import 'media/frame_image.dart';
import 'media/media_source_resolver.dart';
import 'model/video_source.dart';
import 'enum/video_source_type.dart';
import 'domain/player_events.dart';
import 'player/ffi_native_worker.dart';

/// 插件入口：加载原生库并初始化 GStreamer 运行时。
///
/// 推荐在 [runApp] 之前**启动**初始化，但不阻塞首帧；在 [open] / 使用控制器前
/// `await` 完成：
///
/// ```dart
/// Future<void> main() async {
///   WidgetsFlutterBinding.ensureInitialized();
///   final ready = XueHuaVideoPlayer.initialize();
///   runApp(const MyApp());
///   await ready;
/// }
/// ```
class XueHuaVideoPlayer {
  const XueHuaVideoPlayer._();

  static bool _initialized = false;
  static Future<void>? _initializing;

  /// Whether [initialize] has completed successfully in this isolate.
  static bool get isInitialized => _initialized;

  /// 后台初始化 C 运行时（`gst_init`）并预热 FFI worker isolate。
  ///
  /// 不在 UI isolate 上同步阻塞 `gst_init`。幂等；并发调用共享同一 [Future]；
  /// 失败后可重试。
  static Future<void> initialize() {
    if (_initialized) {
      return Future<void>.value();
    }
    return _initializing ??= _initializeOnce();
  }

  static Future<void> _initializeOnce() async {
    final total = Stopwatch()..start();
    NativeCallable<XhvpInitDoneFnFunction>? callable;
    try {
      // Open dylib on this isolate, then start worker in parallel with gst_init.
      XhvpLibrary.instance;
      final workerFuture = xhvpTimedAsync(
        'plugin_worker_spawn',
        FfiNativeWorker.ensureStarted,
      );

      final code = await xhvpTimedAsync('plugin_xhvp_init', () async {
        final done = Completer<int>();
        callable = NativeCallable<XhvpInitDoneFnFunction>.listener((
          Pointer<Void> ctx,
          int result,
        ) {
          if (!done.isCompleted) {
            done.complete(result);
          }
        });
        XhvpLibrary.instance.bindings.xhvp_init_async(
          callable!.nativeFunction,
          nullptr,
        );
        return done.future;
      });

      await workerFuture;

      if (code != 0) {
        throw StateError('xhvp_init failed with code $code');
      }
      _initialized = true;
      xhvpInitTiming('plugin_init', total);
    } catch (_) {
      _initializing = null;
      rethrow;
    } finally {
      callable?.close();
    }
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
          MediaSourceDto_FlutterAsset() => throw StateError(
            'unexpected asset dto',
          ),
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
