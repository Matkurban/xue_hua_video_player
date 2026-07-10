import 'package:flutter/services.dart';

import '../domain/player_events.dart';
import '../player/command_port.dart';
import '../ffi/event_pump.dart';
import '../ffi/xhvp_library.dart';
import 'ffi_native_worker.dart';

/// Production [PlayerCommandPort] backed by the native C player via dart:ffi.
///
/// Blocking `xhvp_runtime_invoke_sync` transport calls run on a single long-lived
/// worker isolate so the Flutter UI isolate never waits on GStreamer preroll and
/// concurrent dispose/load cannot race across ephemeral isolates.
class FfiPlayerCommandPort implements PlayerCommandPort {
  int? _playerId;
  XhvpEventPump? _pump;

  @override
  int? get playerId => _playerId;

  int get _id {
    final id = _playerId;
    if (id == null) {
      throw StateError('PlayerCommandPort used before create()');
    }
    return id;
  }

  void _check(int code, String op) {
    if (code != 0) {
      throw StateError('$op failed with code $code');
    }
  }

  Future<T> _native<T>(FfiRequest request) async {
    final worker = await FfiNativeWorker.ensureStarted();
    return worker.run<T>(request);
  }

  @override
  Future<void> create() async {
    // init + event pump must stay on the root isolate (ReceivePort / NativeCallable).
    XhvpLibrary.instance.init();
    await FfiNativeWorker.ensureStarted();
    final id = XhvpLibrary.instance.bindings.xhvp_player_create();
    if (id == 0) {
      throw StateError('xhvp_player_create failed');
    }
    _playerId = id;
    _pump = XhvpEventPump(id)..start();
  }

  @override
  Stream<PlayerEvent> get events {
    final pump = _pump;
    if (pump == null) {
      throw StateError('PlayerCommandPort used before create()');
    }
    return pump.stream;
  }

  @override
  Future<void> dispose() async {
    final id = _playerId;
    _playerId = null;
    final pump = _pump;
    _pump = null;
    await pump?.dispose();
    if (id != null) {
      final worker = await FfiNativeWorker.ensureStarted();
      await worker.runAfterDrain(FfiDisposeRequest(id));
    }
  }

  @override
  Future<void> loadSource(
    MediaSourceDto source, {
    required bool autoPlay,
  }) async {
    try {
      switch (source) {
        case MediaSourceDto_Uri(:final field0):
          _check(
            await _native<int>(FfiLoadUriRequest(_id, field0, autoPlay)),
            'load_uri($field0)',
          );
        case MediaSourceDto_FlutterAsset(:final field0):
          final data = await rootBundle.load(field0);
          final bytes = data.buffer.asUint8List(
            data.offsetInBytes,
            data.lengthInBytes,
          );
          if (bytes.isEmpty) {
            throw StateError('load_asset($field0) failed: empty asset bytes');
          }
          await _loadAssetBytes(field0, bytes, autoPlay: autoPlay);
      }
    } catch (e) {
      if (e is StateError) {
        rethrow;
      }
      throw StateError('load_source failed: $e');
    }
  }

  Future<void> _loadAssetBytes(
    String assetKey,
    Uint8List bytes, {
    required bool autoPlay,
  }) async {
    _check(
      await _native<int>(
        FfiLoadAssetRequest(_id, assetKey, Uint8List.fromList(bytes), autoPlay),
      ),
      'load_asset($assetKey)',
    );
  }

  @override
  Future<PipelineCapabilitiesDto> getPipelineCapabilities() async {
    final map = await _native<Map<String, bool>>(FfiCapabilitiesRequest(_id));
    return capabilitiesFromMap(map);
  }

  @override
  Future<List<MediaTrack>> getTracks() async {
    final maps = await _native<List<Map<String, Object?>>>(
      FfiGetTracksRequest(_id),
    );
    return tracksFromMaps(maps);
  }

  @override
  Future<void> play() async {
    _check(await _native<int>(FfiIntOpRequest('play', _id)), 'play');
  }

  @override
  Future<void> pause() async {
    _check(await _native<int>(FfiIntOpRequest('pause', _id)), 'pause');
  }

  @override
  Future<void> stop() async {
    _check(await _native<int>(FfiIntOpRequest('stop', _id)), 'stop');
  }

  @override
  Future<void> seek(Duration position) async {
    _check(
      await _native<int>(FfiIntOpRequest('seek', _id, position.inMilliseconds)),
      'seek',
    );
  }

  @override
  Future<void> setVolume(double volume) async {
    _check(
      await _native<int>(FfiIntOpRequest('set_volume', _id, volume)),
      'set_volume',
    );
  }

  @override
  Future<void> setMute(bool mute) async {
    _check(
      await _native<int>(FfiIntOpRequest('set_mute', _id, mute)),
      'set_mute',
    );
  }

  @override
  Future<void> setSpeed(double speed) async {
    _check(
      await _native<int>(FfiIntOpRequest('set_speed', _id, speed)),
      'set_speed',
    );
  }

  @override
  Future<void> setLooping(bool looping) async {
    _check(
      await _native<int>(FfiIntOpRequest('set_looping', _id, looping)),
      'set_looping',
    );
  }

  @override
  Future<void> selectTrack(MediaTrack track, {required bool enable}) async {
    _check(
      await _native<int>(
        FfiIntOpRequest(
          'select_track',
          _id,
          track.id,
          track.trackType.index,
          enable,
        ),
      ),
      'select_track',
    );
  }

  @override
  Future<void> setVideoRotation(int rotateDegrees) async {
    _check(
      await _native<int>(
        FfiIntOpRequest('set_video_rotation', _id, rotateDegrees),
      ),
      'set_video_rotation',
    );
  }

  @override
  Future<void> setAspectRatioMode(AspectRatioMode mode) async {
    _check(
      await _native<int>(
        FfiIntOpRequest('set_aspect_ratio_mode', _id, mode.index),
      ),
      'set_aspect_ratio_mode',
    );
  }
}
