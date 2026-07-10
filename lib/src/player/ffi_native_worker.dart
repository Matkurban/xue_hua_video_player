import 'dart:async';
import 'dart:ffi';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import '../domain/player_events.dart';
import '../ffi/xhvp_library.dart';

/// Long-lived isolate that serializes blocking `xhvp_player_*` FFI calls.
///
/// Create / event pump / [NativeCallable] stay on the root isolate; transport
/// ops go through this queue so dispose/load cannot race across ephemeral
/// [Isolate.run] workers.
class FfiNativeWorker {
  FfiNativeWorker._(this._commands);

  final SendPort _commands;
  final ReceivePort _replies = ReceivePort();
  final Map<int, Completer<Object?>> _pending = {};
  var _nextId = 0;

  static FfiNativeWorker? _instance;
  static Future<FfiNativeWorker>? _starting;

  static Future<FfiNativeWorker> ensureStarted() {
    final existing = _instance;
    if (existing != null) {
      return Future.value(existing);
    }
    return _starting ??= _start();
  }

  static Future<FfiNativeWorker> _start() async {
    final ready = ReceivePort();
    await Isolate.spawn(
      _workerMain,
      ready.sendPort,
      debugName: 'xhvp-ffi-worker',
    );
    final commands = await ready.first as SendPort;
    ready.close();
    final worker = FfiNativeWorker._(commands);
    worker._replies.listen(worker._onReply);
    _instance = worker;
    _starting = null;
    return worker;
  }

  void _onReply(dynamic message) {
    final list = message as List<dynamic>;
    final id = list[0] as int;
    final completer = _pending.remove(id);
    if (completer == null || completer.isCompleted) {
      return;
    }
    final error = list[1];
    if (error != null) {
      completer.completeError(
        error,
        list.length > 2 ? list[2] as StackTrace? : null,
      );
    } else {
      completer.complete(list[2]);
    }
  }

  Future<T> run<T>(FfiRequest request) async {
    final id = _nextId++;
    final completer = Completer<Object?>();
    _pending[id] = completer;
    _commands.send(<Object?>[id, _replies.sendPort, request]);
    return await completer.future as T;
  }

  /// Waits for in-flight requests then runs [request] (typically dispose).
  Future<T> runAfterDrain<T>(FfiRequest request) async {
    while (_pending.isNotEmpty) {
      await Future.wait(_pending.values.map((c) => c.future).toList());
    }
    return run(request);
  }
}

@pragma('vm:entry-point')
void _workerMain(SendPort ready) {
  final commands = ReceivePort();
  ready.send(commands.sendPort);
  // Open native bindings once on this isolate.
  XhvpLibrary.instance;

  commands.listen((message) {
    final list = message as List<dynamic>;
    final id = list[0] as int;
    final reply = list[1] as SendPort;
    final request = list[2] as FfiRequest;
    try {
      final result = request.execute();
      reply.send(<Object?>[id, null, result]);
    } catch (e, st) {
      reply.send(<Object?>[id, e, st]);
    }
  });
}

sealed class FfiRequest {
  const FfiRequest();
  Object? execute();
}

final class FfiDisposeRequest extends FfiRequest {
  const FfiDisposeRequest(this.playerId);
  final int playerId;

  @override
  Object? execute() {
    XhvpLibrary.instance.bindings.xhvp_player_dispose(playerId);
    return null;
  }
}

final class FfiLoadUriRequest extends FfiRequest {
  const FfiLoadUriRequest(this.playerId, this.uri, this.autoPlay);
  final int playerId;
  final String uri;
  final bool autoPlay;

  @override
  Object? execute() {
    final ptr = uri.toNativeUtf8();
    try {
      return XhvpLibrary.instance.bindings.xhvp_player_load_uri(
        playerId,
        ptr.cast(),
        autoPlay,
      );
    } finally {
      malloc.free(ptr);
    }
  }
}

final class FfiLoadAssetRequest extends FfiRequest {
  FfiLoadAssetRequest(this.playerId, this.assetKey, this.bytes, this.autoPlay);
  final int playerId;
  final String assetKey;
  final Uint8List bytes;
  final bool autoPlay;

  @override
  Object? execute() {
    final b = XhvpLibrary.instance.bindings;
    final keyPtr = assetKey.toNativeUtf8();
    final ptr = malloc<Uint8>(bytes.length);
    try {
      ptr.asTypedList(bytes.length).setAll(0, bytes);
      return b.xhvp_player_load_asset(
        playerId,
        keyPtr.cast(),
        nullptr,
        ptr,
        bytes.length,
        autoPlay,
      );
    } finally {
      malloc.free(keyPtr);
      malloc.free(ptr);
    }
  }
}

final class FfiCapabilitiesRequest extends FfiRequest {
  const FfiCapabilitiesRequest(this.playerId);
  final int playerId;

  @override
  Object? execute() {
    final seek = malloc<Bool>();
    final tracks = malloc<Bool>();
    final orientation = malloc<Bool>();
    try {
      final code = XhvpLibrary.instance.bindings.xhvp_player_get_capabilities(
        playerId,
        seek,
        tracks,
        orientation,
      );
      if (code != 0) {
        throw StateError('get_capabilities failed with code $code');
      }
      return <String, bool>{
        'seek': seek.value,
        'tracks': tracks.value,
        'orientation': orientation.value,
      };
    } finally {
      malloc.free(seek);
      malloc.free(tracks);
      malloc.free(orientation);
    }
  }
}

final class FfiGetTracksRequest extends FfiRequest {
  const FfiGetTracksRequest(this.playerId);
  final int playerId;

  @override
  Object? execute() {
    final b = XhvpLibrary.instance.bindings;
    final count = b.xhvp_player_get_track_count(playerId);
    final out = <Map<String, Object?>>[];
    final outId = malloc<Int32>();
    final outType = malloc<Int32>();
    final selected = malloc<Bool>();
    final language = malloc<Char>(32);
    final label = malloc<Char>(128);
    try {
      for (var i = 0; i < count; i++) {
        final rc = b.xhvp_player_get_track(
          playerId,
          i,
          outId,
          outType,
          language,
          32,
          label,
          128,
          selected,
        );
        if (rc != 0) {
          continue;
        }
        out.add(<String, Object?>{
          'id': outId.value,
          'type': outType.value,
          'language': language.cast<Utf8>().toDartString(),
          'label': label.cast<Utf8>().toDartString(),
          'selected': selected.value,
        });
      }
    } finally {
      malloc.free(outId);
      malloc.free(outType);
      malloc.free(selected);
      malloc.free(language);
      malloc.free(label);
    }
    return out;
  }
}

final class FfiIntOpRequest extends FfiRequest {
  const FfiIntOpRequest(
    this.op,
    this.playerId, [
    this.arg0,
    this.arg1,
    this.arg2,
  ]);
  final String op;
  final int playerId;
  final Object? arg0;
  final Object? arg1;
  final Object? arg2;

  @override
  Object? execute() {
    final b = XhvpLibrary.instance.bindings;
    switch (op) {
      case 'play':
        return b.xhvp_player_play(playerId);
      case 'pause':
        return b.xhvp_player_pause(playerId);
      case 'stop':
        return b.xhvp_player_stop(playerId);
      case 'seek':
        return b.xhvp_player_seek(playerId, arg0! as int);
      case 'set_volume':
        return b.xhvp_player_set_volume(playerId, arg0! as double);
      case 'set_mute':
        return b.xhvp_player_set_mute(playerId, arg0! as bool);
      case 'set_speed':
        return b.xhvp_player_set_speed(playerId, arg0! as double);
      case 'set_looping':
        return b.xhvp_player_set_looping(playerId, arg0! as bool);
      case 'select_track':
        return b.xhvp_player_select_track(
          playerId,
          arg0! as int,
          arg1! as int,
          arg2! as bool,
        );
      case 'set_video_rotation':
        return b.xhvp_player_set_video_rotation(playerId, arg0! as int);
      case 'set_aspect_ratio_mode':
        return b.xhvp_player_set_aspect_ratio_mode(playerId, arg0! as int);
      default:
        throw StateError('Unknown FFI op: $op');
    }
  }
}

PipelineCapabilitiesDto capabilitiesFromMap(Map<String, bool> map) {
  return PipelineCapabilitiesDto(
    seek: map['seek'] ?? false,
    tracks: map['tracks'] ?? false,
    orientation: map['orientation'] ?? false,
  );
}

List<MediaTrack> tracksFromMaps(List<Map<String, Object?>> maps) {
  final types = TrackType.values;
  return maps.map((m) {
    final typeIdx = m['type']! as int;
    return MediaTrack(
      id: m['id']! as int,
      trackType: typeIdx >= 0 && typeIdx < types.length
          ? types[typeIdx]
          : TrackType.audio,
      language: m['language']! as String,
      label: m['label']! as String,
      selected: m['selected']! as bool,
    );
  }).toList();
}
