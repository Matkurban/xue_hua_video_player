import 'package:flutter/widgets.dart';
import 'package:signals/signals_flutter.dart';

import '../rust/player_events.dart';

/// Playback state driven by [PlayerEvent] reducers and facade-coordinated previews.
class PlayerStateStore {
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
  final FlutterSignal<bool> _supportsTracks = signal(true);
  final FlutterSignal<bool> _supportsOrientation = signal(true);

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
  ReadonlySignal<bool> get supportsTracks => _supportsTracks;
  ReadonlySignal<bool> get supportsOrientation => _supportsOrientation;

  void markInitialized({required int playerId}) {
    _playerId.value = playerId;
    _initialized.value = true;
  }

  void resetForOpen() {
    batch(() {
      _error.value = null;
      _bufferingPercent.value = 100;
      _videoSize.value = Size.zero;
      _videoMetadata.value = null;
      _tracks.value = const [];
      _speed.value = 1.0;
    });
  }

  void apply(PlayerEvent event) {
    switch (event.kind) {
      case PlayerEventKind.durationChanged:
        _duration.value = Duration(milliseconds: event.durationMs);
      case PlayerEventKind.positionChanged:
        _position.value = Duration(milliseconds: event.positionMs);
      case PlayerEventKind.videoSize:
        _videoSize.value = Size(
          event.width.toDouble(),
          event.height.toDouble(),
        );
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
      case PlayerEventKind.stateChanged:
        _state.value = event.state;
        if (event.state == PlayerState.playing) {
          _bufferingPercent.value = 100;
        }
      case PlayerEventKind.buffering:
        _bufferingPercent.value = event.bufferingPercent;
      case PlayerEventKind.eos:
        batch(() {
          _state.value = PlayerState.completed;
          _position.value = _duration.value;
        });
      case PlayerEventKind.error:
        batch(() {
          _error.value = event.message;
          _state.value = PlayerState.error;
        });
      case PlayerEventKind.tracksChanged:
        break;
    }
  }

  void applyError(String message) {
    batch(() {
      _error.value = message;
      _state.value = PlayerState.error;
    });
  }

  void setPipelineCapabilities(PipelineCapabilitiesDto caps) {
    _isSeekable.value = caps.seek;
    _supportsTracks.value = caps.tracks;
    _supportsOrientation.value = caps.orientation;
  }

  void setTracks(List<MediaTrack> tracks) {
    _tracks.value = tracks;
  }

  void previewSeek(Duration position, {required bool showBuffering}) {
    _position.value = position;
    if (showBuffering) {
      _state.value = PlayerState.buffering;
    }
  }

  void previewVolume(double volume) {
    final v = volume.clamp(0.0, 1.0);
    _volume.value = v;
    if (v > 0 && _muted.value) {
      _muted.value = false;
    }
  }

  void previewMuted(bool muted) {
    _muted.value = muted;
  }

  void previewSpeed(double speed) {
    _speed.value = speed <= 0 ? 1.0 : speed;
  }

  void previewLooping(bool looping) {
    _looping.value = looping;
  }

  void dispose() {
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
    _playerId.dispose();
    _initialized.dispose();
    _tracks.dispose();
    _videoMetadata.dispose();
    _isSeekable.dispose();
    _supportsTracks.dispose();
    _supportsOrientation.dispose();
  }
}
