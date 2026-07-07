import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:signals/signals_flutter.dart';

import 'media/media_source_resolver.dart';
import 'model/video_source.dart';
import 'controls/playback_controls_model.dart';
import 'player/command_port.dart';
import 'player/state_store.dart';
import 'rust/player_events.dart';

export 'rust/player_events.dart'
    show
        AspectRatioMode,
        MediaTrack,
        PipelineCapabilitiesDto,
        PlayerState,
        PlayerEvent,
        PlayerEventKind,
        TrackType,
        VideoMetadata,
        VideoOrientationConfig;
export 'model/video_source.dart';

/// Drives a single GStreamer-backed player living in Rust and exposes its
/// state to Flutter widgets through fine-grained [signals].
class XueHuaPlayerController implements PlaybackControlsModel {
  XueHuaPlayerController({
    PlayerCommandPort? port,
    MediaSourceResolver? mediaSourceResolver,
    PlayerStateStore? store,
  }) : _port = port ?? ProductionPlayerCommandPort(),
       _mediaSourceResolver =
           mediaSourceResolver ?? const MediaSourceResolver(),
       _store = store ?? PlayerStateStore();

  final PlayerCommandPort _port;
  final MediaSourceResolver _mediaSourceResolver;
  final PlayerStateStore _store;

  StreamSubscription<PlayerEvent>? _sub;
  bool _disposed = false;

  ReadonlySignal<bool> get initialized => _store.initialized;
  ReadonlySignal<int?> get playerId => _store.playerId;
  @override
  ReadonlySignal<PlayerState> get state => _store.state;
  @override
  ReadonlySignal<Duration> get position => _store.position;
  @override
  ReadonlySignal<Duration> get duration => _store.duration;
  ReadonlySignal<Size> get videoSize => _store.videoSize;
  ReadonlySignal<int> get bufferingPercent => _store.bufferingPercent;
  @override
  ReadonlySignal<double> get volume => _store.volume;
  @override
  ReadonlySignal<double> get speed => _store.speed;
  @override
  ReadonlySignal<bool> get looping => _store.looping;
  @override
  ReadonlySignal<bool> get muted => _store.muted;
  ReadonlySignal<String?> get error => _store.error;
  ReadonlySignal<List<MediaTrack>> get tracks => _store.tracks;
  ReadonlySignal<VideoMetadata?> get videoMetadata => _store.videoMetadata;
  @override
  ReadonlySignal<bool> get isSeekable => _store.isSeekable;
  ReadonlySignal<bool> get supportsTracks => _store.supportsTracks;
  ReadonlySignal<bool> get supportsOrientation => _store.supportsOrientation;
  @override
  ReadonlySignal<bool> get isPlaying => _store.isPlaying;
  ReadonlySignal<bool> get isCompleted => _store.isCompleted;
  ReadonlySignal<double> get aspectRatio => _store.aspectRatio;

  Future<void> initialize() async {
    if (_store.initialized.value) return;
    await _port.create();
    final id = _port.playerId;
    if (id == null) {
      throw StateError('PlayerCommandPort.create() did not assign playerId');
    }
    _store.markInitialized(playerId: id);
    _sub = _port.events.listen(
      _onEvent,
      onError: (Object e) => _store.applyError(e.toString()),
    );
  }

  void _onEvent(PlayerEvent event) {
    if (_disposed) return;
    if (event.kind == PlayerEventKind.tracksChanged) {
      unawaited(_refreshTracksFromPort());
      return;
    }
    _store.apply(event);
  }

  Future<void> _guard(Future<void> Function() action) async {
    try {
      await action();
    } catch (e) {
      _store.applyError(e.toString());
    }
  }

  /// Loads [source] via the unified Rust media resolver.
  Future<void> open(VideoSource source, {bool autoPlay = false}) async {
    _store.resetForOpen();
    await _guard(() async {
      await _port.loadSource(
        _mediaSourceResolver.resolve(source),
        autoPlay: autoPlay,
      );
      _store.setPipelineCapabilities(await _port.getPipelineCapabilities());
      await _refreshTracksFromPort();
    });
  }

  Future<void> play() => _guard(_port.play);

  Future<void> pause() => _guard(_port.pause);

  Future<void> stop() => _guard(_port.stop);

  @override
  Future<void> togglePlayPause() => _store.isPlaying.value ? pause() : play();

  @override
  Future<void> seek(Duration position) async {
    _store.previewSeek(position, showBuffering: _store.isPlaying.value);
    await _guard(() => _port.seek(position));
  }

  Future<void> setVolume(double volume) async {
    _store.previewVolume(volume);
    await _guard(() => _port.setVolume(_store.volume.value));
  }

  Future<void> setMuted(bool muted) async {
    _store.previewMuted(muted);
    await _guard(() => _port.setMute(muted));
  }

  @override
  Future<void> toggleMuted() => setMuted(!_store.muted.value);

  @override
  Future<void> setSpeed(double speed) async {
    _store.previewSpeed(speed);
    await _guard(() => _port.setSpeed(_store.speed.value));
  }

  @override
  Future<void> setLooping(bool looping) async {
    _store.previewLooping(looping);
    await _guard(() => _port.setLooping(looping));
  }

  Future<void> refreshTracks() => _refreshTracksFromPort();

  Future<void> _refreshTracksFromPort() async {
    try {
      _store.setTracks(await _port.getTracks());
    } catch (e) {
      _store.applyError(e.toString());
    }
  }

  Future<void> selectTrack(MediaTrack track, {bool enable = true}) =>
      _guard(() => _port.selectTrack(track, enable: enable));

  Future<void> setVideoOrientation(VideoOrientationConfig config) =>
      _guard(() => _port.setVideoOrientation(config));

  Future<void> setAspectRatioMode(AspectRatioMode mode) =>
      _guard(() => _port.setAspectRatioMode(mode));

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _sub?.cancel();
    await _port.dispose();
    _store.dispose();
  }
}
