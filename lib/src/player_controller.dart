import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart' show rootBundle;
import 'package:flutter/widgets.dart';
import 'package:irondash_engine_context/irondash_engine_context.dart';
import 'package:signals/signals_flutter.dart';

import 'rust/api/player.dart' as rust;
import 'rust/player.dart';
import 'video_source.dart';

export 'rust/player.dart' show PlayerState, PlayerEvent, PlayerEventKind;
export 'video_source.dart';

/// Drives a single GStreamer-backed video player living in Rust and exposes its
/// state to Flutter widgets through fine-grained [signals].
///
/// Every piece of state is a [ReadonlySignal]; read `.value` inside a
/// `SignalBuilder` (or `Watch`) so only the widgets that depend on a given
/// field rebuild when it changes.
///
/// Typical usage:
/// ```dart
/// final controller = XueHuaPlayerController();
/// await controller.initialize();
/// await controller.open(VideoSource.network('https://.../video.mp4'), autoPlay: true);
/// // ...
/// XueHuaVideoView(controller: controller);
/// // ...
/// await controller.dispose();
/// ```
class XueHuaPlayerController {
  int? _playerId;
  StreamSubscription<PlayerEvent>? _sub;
  bool _disposed = false;

  final FlutterSignal<PlayerState> _state = signal(PlayerState.idle);
  final FlutterSignal<Duration> _position = signal(Duration.zero);
  final FlutterSignal<Duration> _duration = signal(Duration.zero);
  final FlutterSignal<Size> _videoSize = signal(Size.zero);
  final FlutterSignal<int> _bufferingPercent = signal(100);
  final FlutterSignal<double> _volume = signal(1.0);
  final FlutterSignal<double> _speed = signal(1.0);
  final FlutterSignal<bool> _looping = signal(false);
  final FlutterSignal<bool> _muted = signal(false);
  final FlutterSignal<String?> _error = signal<String?>(null);
  final FlutterSignal<int?> _textureId = signal<int?>(null);
  final FlutterSignal<bool> _initialized = signal(false);

  /// Whether [initialize] has completed.
  ReadonlySignal<bool> get initialized => _initialized;

  /// The Flutter external texture id, or null before [initialize].
  ReadonlySignal<int?> get textureId => _textureId;

  /// High-level playback state reported by the pipeline.
  ReadonlySignal<PlayerState> get state => _state;

  /// Latest known playback position.
  ReadonlySignal<Duration> get position => _position;

  /// Media duration, or [Duration.zero] until known.
  ReadonlySignal<Duration> get duration => _duration;

  /// Decoded video size in pixels, or [Size.zero] until the first frame.
  ReadonlySignal<Size> get videoSize => _videoSize;

  /// Buffering fill level in the range `0..100`.
  ReadonlySignal<int> get bufferingPercent => _bufferingPercent;

  /// Current volume in the range `0.0..1.0`.
  ReadonlySignal<double> get volume => _volume;

  /// Current playback speed multiplier.
  ReadonlySignal<double> get speed => _speed;

  /// Whether the media loops when it reaches the end.
  ReadonlySignal<bool> get looping => _looping;

  /// Whether audio output is muted.
  ReadonlySignal<bool> get muted => _muted;

  /// Last error message, or null when healthy.
  ReadonlySignal<String?> get error => _error;

  /// Whether the pipeline is currently playing.
  late final ReadonlySignal<bool> isPlaying = computed(() => _state.value == PlayerState.playing);

  /// Whether playback reached the end of the media.
  late final ReadonlySignal<bool> isCompleted = computed(
    () => _state.value == PlayerState.completed,
  );

  /// Aspect ratio of the decoded video, or 16:9 until the size is known.
  late final ReadonlySignal<double> aspectRatio = computed(() {
    final s = _videoSize.value;
    return (s.width > 0 && s.height > 0) ? s.width / s.height : 16 / 9;
  });

  /// Creates the native player and its texture, and subscribes to events.
  Future<void> initialize() async {
    if (_initialized.value) return;
    final handle = await EngineContext.instance.getEngineHandle();
    final result = await rust.createPlayer(engineHandle: handle);
    _playerId = result.playerId;
    _textureId.value = result.textureId;
    _sub = rust
        .playerEventStream(playerId: _playerId!)
        .listen(_onEvent, onError: (Object e) => _error.value = e.toString());
    _initialized.value = true;
  }

  void _onEvent(PlayerEvent event) {
    if (_disposed) return;
    switch (event.kind) {
      case PlayerEventKind.durationChanged:
        _duration.value = Duration(milliseconds: event.durationMs);
        break;
      case PlayerEventKind.positionChanged:
        _position.value = Duration(milliseconds: event.positionMs);
        break;
      case PlayerEventKind.videoSize:
        _videoSize.value = Size(event.width.toDouble(), event.height.toDouble());
        break;
      case PlayerEventKind.stateChanged:
        _state.value = event.state;
        break;
      case PlayerEventKind.buffering:
        _bufferingPercent.value = event.bufferingPercent;
        break;
      case PlayerEventKind.eos:
        batch(() {
          _state.value = PlayerState.completed;
          _position.value = _duration.value;
        });
        break;
      case PlayerEventKind.error:
        batch(() {
          _error.value = event.message;
          _state.value = PlayerState.error;
        });
        break;
    }
  }

  int get _id {
    final id = _playerId;
    if (id == null) {
      throw StateError('XueHuaPlayerController used before initialize()');
    }
    return id;
  }

  /// Runs a control call, converting failures into an [error] state update
  /// instead of an unhandled exception (playback errors are also delivered
  /// asynchronously via the event stream).
  Future<void> _guard(Future<void> Function() action) async {
    try {
      await action();
    } catch (e) {
      _error.value = e.toString();
    }
  }

  /// Loads [source]. Pass a [VideoSource] describing a network URL, a local
  /// file, or a bundled asset; assets are copied to a temporary file first
  /// since GStreamer can only read files and URLs.
  Future<void> open(VideoSource source, {bool autoPlay = false}) async {
    _error.value = null;
    await _guard(() async {
      final uri = await _resolveGstUri(source);
      await rust.playerSetSource(playerId: _id, uri: uri);
      if (autoPlay) await rust.playerPlay(playerId: _id);
    });
  }

  Future<void> play() => _guard(() => rust.playerPlay(playerId: _id));

  Future<void> pause() => _guard(() => rust.playerPause(playerId: _id));

  Future<void> stop() => _guard(() => rust.playerStop(playerId: _id));

  /// Toggles between play and pause based on the current [state].
  Future<void> togglePlayPause() => isPlaying.value ? pause() : play();

  Future<void> seek(Duration position) =>
      _guard(() => rust.playerSeek(playerId: _id, positionMs: position.inMilliseconds));

  Future<void> setVolume(double volume) async {
    final v = volume.clamp(0.0, 1.0);
    _volume.value = v;
    if (v > 0 && _muted.value) _muted.value = false;
    await _guard(() => rust.playerSetVolume(playerId: _id, volume: v));
  }

  Future<void> setMuted(bool muted) async {
    _muted.value = muted;
    await _guard(() => rust.playerSetMute(playerId: _id, mute: muted));
  }

  /// Flips the current mute state.
  Future<void> toggleMuted() => setMuted(!_muted.value);

  Future<void> setSpeed(double speed) async {
    final s = speed <= 0 ? 1.0 : speed;
    _speed.value = s;
    await _guard(() => rust.playerSetSpeed(playerId: _id, speed: s));
  }

  Future<void> setLooping(bool looping) async {
    _looping.value = looping;
    await _guard(() => rust.playerSetLooping(playerId: _id, looping: looping));
  }

  /// Queries the current position directly from the pipeline.
  Future<Duration> queryPosition() async =>
      Duration(milliseconds: await rust.playerPosition(playerId: _id));

  /// Queries the media duration directly from the pipeline.
  Future<Duration> queryDuration() async =>
      Duration(milliseconds: await rust.playerDuration(playerId: _id));

  /// Cancels the event stream, disposes the native player, and releases every
  /// signal owned by this controller.
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _sub?.cancel();
    final id = _playerId;
    if (id != null) {
      await rust.disposePlayer(playerId: id);
    }
    isPlaying.dispose();
    isCompleted.dispose();
    aspectRatio.dispose();
    _state.dispose();
    _position.dispose();
    _duration.dispose();
    _videoSize.dispose();
    _bufferingPercent.dispose();
    _volume.dispose();
    _speed.dispose();
    _looping.dispose();
    _muted.dispose();
    _error.dispose();
    _textureId.dispose();
    _initialized.dispose();
  }

  /// Turns a [VideoSource] into a URI GStreamer can open.
  static Future<String> _resolveGstUri(VideoSource source) async {
    switch (source.type) {
      case VideoSourceType.network:
        return source.uri.trim();
      case VideoSourceType.file:
        final trimmed = source.uri.trim();
        final parsed = Uri.tryParse(trimmed);
        if (parsed != null && parsed.hasScheme) {
          return trimmed;
        }
        return Uri.file(trimmed).toString();
      case VideoSourceType.asset:
        final file = await _materializeAsset(source.uri);
        return Uri.file(file.path).toString();
    }
  }

  /// Copies a Flutter asset to a cached temp file so GStreamer can read it,
  /// reusing the existing file when its byte length already matches.
  static Future<File> _materializeAsset(String assetKey) async {
    final data = await rootBundle.load(assetKey);
    final bytes = data.buffer.asUint8List(
      data.offsetInBytes,
      data.lengthInBytes,
    );
    final dir = Directory('${Directory.systemTemp.path}/xhvp_assets');
    await dir.create(recursive: true);
    final name = assetKey.replaceAll(RegExp(r'[^\w.]+'), '_');
    final file = File('${dir.path}/$name');
    if (!file.existsSync() || file.lengthSync() != bytes.length) {
      await file.writeAsBytes(bytes, flush: true);
    }
    return file;
  }
}
