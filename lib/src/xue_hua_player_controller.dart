import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:signals/signals_flutter.dart';

import 'enum/video_source_type.dart';
import 'rust/api/player.dart' as rust;
import 'rust/player_events.dart';
import 'model/video_source.dart';

export 'rust/player_events.dart'
    show
        AspectRatioMode,
        MediaTrack,
        PlayerState,
        PlayerEvent,
        PlayerEventKind,
        TrackType,
        VideoMetadata,
        VideoOrientationConfig;
export 'model/video_source.dart';

/// Drives a single GStreamer-backed player living in Rust and exposes its
/// state to Flutter widgets through fine-grained [signals].
class XueHuaPlayerController {
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
  final FlutterSignal<int?> _playerId = signal<int?>(null);
  final FlutterSignal<bool> _initialized = signal(false);
  final FlutterSignal<List<MediaTrack>> _tracks = signal(const []);
  final FlutterSignal<VideoMetadata?> _videoMetadata = signal(null);
  final FlutterSignal<bool> _isSeekable = signal(true);

  ReadonlySignal<bool> get initialized => _initialized;
  ReadonlySignal<int?> get playerId => _playerId;
  ReadonlySignal<PlayerState> get state => _state;
  ReadonlySignal<Duration> get position => _position;
  ReadonlySignal<Duration> get duration => _duration;
  ReadonlySignal<Size> get videoSize => _videoSize;
  ReadonlySignal<int> get bufferingPercent => _bufferingPercent;
  ReadonlySignal<double> get volume => _volume;
  ReadonlySignal<double> get speed => _speed;
  ReadonlySignal<bool> get looping => _looping;
  ReadonlySignal<bool> get muted => _muted;
  ReadonlySignal<String?> get error => _error;
  ReadonlySignal<List<MediaTrack>> get tracks => _tracks;
  ReadonlySignal<VideoMetadata?> get videoMetadata => _videoMetadata;
  ReadonlySignal<bool> get isSeekable => _isSeekable;

  late final ReadonlySignal<bool> isPlaying = computed(
    () => _state.value == PlayerState.playing,
  );

  late final ReadonlySignal<bool> isCompleted = computed(
    () => _state.value == PlayerState.completed,
  );

  late final ReadonlySignal<double> aspectRatio = computed(() {
    final meta = _videoMetadata.value;
    if (meta != null &&
        meta.displayAspectWidth > 0 &&
        meta.displayAspectHeight > 0) {
      return meta.displayAspectWidth / meta.displayAspectHeight;
    }
    final s = _videoSize.value;
    return (s.width > 0 && s.height > 0) ? s.width / s.height : 16 / 9;
  });

  late final ReadonlySignal<double> displayAspectRatio = aspectRatio;

  Future<void> initialize() async {
    if (_initialized.value) return;
    final result = await rust.createPlayer();
    _playerId.value = result.playerId;
    _sub = rust
        .playerEventStream(playerId: result.playerId)
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
        _videoSize.value = Size(
          event.width.toDouble(),
          event.height.toDouble(),
        );
        break;
      case PlayerEventKind.metadataChanged:
        _videoMetadata.value = VideoMetadata(
          width: event.width,
          height: event.height,
          fps: event.fps,
          pixelAspectWidth: event.pixelAspectWidth,
          pixelAspectHeight: event.pixelAspectHeight,
          displayAspectWidth: event.displayAspectWidth,
          displayAspectHeight: event.displayAspectHeight,
          interlaced: event.interlaced,
          colorMatrix: event.colorMatrix,
          colorRange: event.colorRange,
          hdrFormat: event.hdrFormat,
        );
        _isSeekable.value = event.isSeekable;
        break;
      case PlayerEventKind.stateChanged:
        _state.value = event.state;
        break;
      case PlayerEventKind.buffering:
        batch(() {
          _bufferingPercent.value = event.bufferingPercent;
          if (event.bufferingPercent < 100) {
            _state.value = PlayerState.buffering;
          }
        });
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
      case PlayerEventKind.tracksChanged:
        unawaited(refreshTracks());
        break;
    }
  }

  int get _id {
    final id = _playerId.value;
    if (id == null) {
      throw StateError('XueHuaPlayerController used before initialize()');
    }
    return id;
  }

  Future<void> _guard(Future<void> Function() action) async {
    try {
      await action();
    } catch (e) {
      _error.value = e.toString();
    }
  }

  MediaSourceDto _toMediaSourceDto(VideoSource source) {
    switch (source.type) {
      case VideoSourceType.asset:
        return MediaSourceDto.flutterAsset(source.uri.trim());
      case VideoSourceType.network:
      case VideoSourceType.file:
        return MediaSourceDto.uri(_resolveGstUri(source));
    }
  }

  /// Loads [source] via the unified Rust media resolver.
  Future<void> open(VideoSource source, {bool autoPlay = false}) async {
    batch(() {
      _error.value = null;
      _bufferingPercent.value = 100;
      _videoSize.value = Size.zero;
      _videoMetadata.value = null;
      _tracks.value = const [];
      _speed.value = 1.0;
      _state.value = PlayerState.buffering;
    });
    await _guard(() async {
      await rust.playerLoadSource(
        playerId: _id,
        source: _toMediaSourceDto(source),
      );
      _isSeekable.value = await rust.playerIsSeekable(playerId: _id);
      await refreshTracks();
      if (autoPlay) await rust.playerPlay(playerId: _id);
    });
  }

  Future<void> play() => _guard(() => rust.playerPlay(playerId: _id));

  Future<void> pause() => _guard(() => rust.playerPause(playerId: _id));

  Future<void> stop() => _guard(() => rust.playerStop(playerId: _id));

  Future<void> togglePlayPause() => isPlaying.value ? pause() : play();

  Future<void> seek(Duration position) => _guard(
    () => rust.playerSeek(playerId: _id, positionMs: position.inMilliseconds),
  );

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

  Future<void> refreshTracks() async {
    final list = await rust.playerGetTracks(playerId: _id);
    _tracks.value = list;
  }

  Future<void> selectTrack(MediaTrack track, {bool enable = true}) => _guard(
    () => rust.playerSelectTrack(
      playerId: _id,
      trackId: track.id,
      trackType: track.trackType,
      enable: enable,
    ),
  );

  Future<VideoMetadata?> queryVideoMetadata() async {
    final meta = await rust.playerGetVideoMetadata(playerId: _id);
    _videoMetadata.value = meta;
    return meta;
  }

  Future<void> setVideoOrientation(VideoOrientationConfig config) => _guard(
    () => rust.playerSetVideoOrientation(playerId: _id, config: config),
  );

  Future<void> setAspectRatioMode(AspectRatioMode mode) => _guard(
    () => rust.playerSetAspectRatioMode(playerId: _id, mode: mode),
  );

  Future<Duration> queryPosition() async =>
      Duration(milliseconds: await rust.playerPosition(playerId: _id));

  Future<Duration> queryDuration() async =>
      Duration(milliseconds: await rust.playerDuration(playerId: _id));

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _sub?.cancel();
    final id = _playerId.value;
    if (id != null) {
      await rust.disposePlayer(playerId: id);
    }
    isPlaying.dispose();
    isCompleted.dispose();
    aspectRatio.dispose();
    displayAspectRatio.dispose();
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
    _playerId.dispose();
    _initialized.dispose();
    _tracks.dispose();
    _videoMetadata.dispose();
    _isSeekable.dispose();
  }

  static String _resolveGstUri(VideoSource source) {
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
        return source.uri.trim();
    }
  }
}
