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
/// [initialize] 只做 kickoff（打开 dylib、启动后台 `gst_init` / worker），目标
/// <50ms，不阻塞首帧。真正就绪由 [ensureReady] 等待；`create` / `open` /
/// [captureThumbnail] 会自动调用。
///
/// ```dart
/// Future<void> main() async {
///   WidgetsFlutterBinding.ensureInitialized();
///   unawaited(XueHuaVideoPlayer.initialize()); // 或 await（仅等 kickoff）
///   runApp(const MyApp());
/// }
/// ```
class XueHuaVideoPlayer {
  const XueHuaVideoPlayer._();

  static bool _initialized = false;
  static Future<void>? _kickoff;
  static Future<void>? _ready;

  /// Whether the native runtime is ready (`gst_init` + worker) in this isolate.
  ///
  /// May still be `false` right after [initialize] returns; use [ensureReady]
  /// when you need a hard wait. Controller / thumbnail paths await this for you.
  static bool get isInitialized => _initialized;

  /// Kick off C runtime init and FFI worker spawn without waiting for them.
  ///
  /// Idempotent; concurrent calls share the same kickoff [Future]. Does not
  /// block the UI isolate on `gst_init`. Failures during dylib open clear the
  /// kickoff so a later call can retry.
  static Future<void> initialize() {
    if (_initialized) {
      return Future<void>.value();
    }
    return _kickoff ??= _kickoffOnce();
  }

  /// Await full runtime readiness (`gst_init` success + worker started).
  ///
  /// Starts [initialize] if needed. Idempotent; failures clear state so a later
  /// call can retry.
  static Future<void> ensureReady() {
    if (_initialized) {
      return Future<void>.value();
    }
    // Ensure kickoff has scheduled `_ready` (or schedule ready directly).
    _kickoff ??= _kickoffOnce();
    return _ready ??= _readyOnce();
  }

  static Future<void> _kickoffOnce() async {
    final total = Stopwatch()..start();
    try {
      XhvpLibrary.instance;
      // Schedule ready work; do not await gst_init / worker spawn here.
      _ready ??= _readyOnce();
      xhvpInitTiming('plugin_kickoff', total);
    } catch (_) {
      _kickoff = null;
      _ready = null;
      rethrow;
    }
  }

  static Future<void> _readyOnce() async {
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
      _ready = null;
      _kickoff = null;
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
  /// 须先 [initialize]（或由本方法内部 [ensureReady]）。不占用播放中的 controller。
  static Future<Uint8List> captureThumbnail(
    VideoSource source, {
    Duration? at,
    int maxWidth = 320,
  }) async {
    await ensureReady();
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
