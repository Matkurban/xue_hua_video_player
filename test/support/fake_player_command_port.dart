import 'dart:async';

import 'package:xue_hua_video_player/src/player/command_port.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';

/// Test double for [PlayerCommandPort] — the Dart/Rust seam under [PlaybackSession].
class FakePlayerCommandPort implements PlayerCommandPort {
  FakePlayerCommandPort({
    this.failCreate = false,
    this.failLoad = false,
    this.failSeek = false,
    this.seekable = true,
    List<MediaTrack>? tracksToReturn,
  }) : tracksToReturn = tracksToReturn ?? const [];

  final bool failCreate;
  final bool failLoad;
  final bool failSeek;
  final bool seekable;

  List<MediaTrack> tracksToReturn;

  int? _playerId;
  final StreamController<PlayerEvent> _events = StreamController.broadcast();

  MediaSourceDto? lastLoadedSource;
  Duration? lastSeekPosition;
  double? lastVolume;
  bool? lastMute;
  AspectRatioMode? lastAspectRatioMode;
  VideoOrientationConfig? lastVideoOrientation;
  int setAspectRatioModeCallCount = 0;
  int playCallCount = 0;
  int pauseCallCount = 0;
  int getTracksCallCount = 0;

  @override
  int? get playerId => _playerId;

  @override
  Future<void> create() async {
    if (failCreate) {
      throw StateError('create failed');
    }
    _playerId = 42;
  }

  @override
  Stream<PlayerEvent> get events => _events.stream;

  @override
  Future<void> dispose() async {
    _playerId = null;
    await _events.close();
  }

  @override
  Future<void> loadSource(
    MediaSourceDto source, {
    required bool autoPlay,
  }) async {
    if (failLoad) {
      throw StateError('load failed');
    }
    lastLoadedSource = source;
  }

  @override
  Future<PipelineCapabilitiesDto> getPipelineCapabilities() async {
    return PipelineCapabilitiesDto(
      seek: seekable,
      tracks: false,
      orientation: true,
    );
  }

  @override
  Future<List<MediaTrack>> getTracks() async {
    getTracksCallCount++;
    return tracksToReturn;
  }

  @override
  Future<void> play() async {
    playCallCount++;
  }

  @override
  Future<void> pause() async {
    pauseCallCount++;
  }

  @override
  Future<void> stop() async {}

  @override
  Future<void> seek(Duration position) async {
    if (failSeek) {
      throw StateError('seek failed');
    }
    lastSeekPosition = position;
  }

  @override
  Future<void> setVolume(double volume) async {
    lastVolume = volume;
  }

  @override
  Future<void> setMute(bool mute) async {
    lastMute = mute;
  }

  @override
  Future<void> setSpeed(double speed) async {}

  @override
  Future<void> setLooping(bool looping) async {}

  @override
  Future<void> selectTrack(MediaTrack track, {required bool enable}) async {}

  @override
  Future<void> setVideoOrientation(VideoOrientationConfig config) async {
    lastVideoOrientation = config;
  }

  @override
  Future<void> setAspectRatioMode(AspectRatioMode mode) async {
    setAspectRatioModeCallCount++;
    lastAspectRatioMode = mode;
  }

  void emit(PlayerEvent event) {
    _events.add(event);
  }
}
