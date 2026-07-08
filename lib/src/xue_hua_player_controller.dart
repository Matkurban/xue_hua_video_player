import 'package:flutter/widgets.dart';
import 'package:signals/signals_flutter.dart';

import 'controls/playback_controls_model.dart';
import 'presentation/playback_presentation_model.dart';
import 'media/media_source_resolver.dart';
import 'model/video_source.dart';
import 'player/command_port.dart';
import 'player/playback_session.dart';
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

/// Public facade for a single GStreamer-backed player.
///
/// Delegates orchestration to [PlaybackSession]; implements [PlaybackControlsModel]
/// and [PlaybackPresentationModel] for built-in view / controls.
class XueHuaPlayerController
    implements PlaybackControlsModel, PlaybackPresentationModel {
  XueHuaPlayerController({
    PlayerCommandPort? port,
    MediaSourceResolver? mediaSourceResolver,
    PlaybackSession? session,
  }) : _session =
           session ??
           PlaybackSession(
             port: port,
             mediaSourceResolver: mediaSourceResolver,
           );

  final PlaybackSession _session;

  @override
  ReadonlySignal<bool> get initialized => _session.initialized;
  @override
  ReadonlySignal<int?> get playerId => _session.playerId;
  @override
  ReadonlySignal<PlayerState> get state => _session.state;
  @override
  ReadonlySignal<Duration> get position => _session.position;
  @override
  ReadonlySignal<Duration> get duration => _session.duration;
  ReadonlySignal<Size> get videoSize => _session.videoSize;
  @override
  ReadonlySignal<int> get bufferingPercent => _session.bufferingPercent;
  @override
  ReadonlySignal<double> get volume => _session.volume;
  @override
  ReadonlySignal<double> get speed => _session.speed;
  @override
  ReadonlySignal<bool> get looping => _session.looping;
  @override
  ReadonlySignal<bool> get muted => _session.muted;
  ReadonlySignal<String?> get error => _session.error;
  ReadonlySignal<List<MediaTrack>> get tracks => _session.tracks;
  ReadonlySignal<VideoMetadata?> get videoMetadata => _session.videoMetadata;
  @override
  ReadonlySignal<bool> get isSeekable => _session.isSeekable;
  ReadonlySignal<bool> get supportsTracks => _session.supportsTracks;
  ReadonlySignal<bool> get supportsOrientation => _session.supportsOrientation;
  @override
  ReadonlySignal<bool> get isPlaying => _session.isPlaying;
  ReadonlySignal<bool> get isCompleted => _session.isCompleted;
  @override
  ReadonlySignal<double> get aspectRatio => _session.aspectRatio;

  Future<void> initialize() => _session.initialize();

  Future<void> open(VideoSource source, {bool autoPlay = false}) =>
      _session.open(source, autoPlay: autoPlay);

  Future<void> play() => _session.play();

  Future<void> pause() => _session.pause();

  Future<void> stop() => _session.stop();

  @override
  Future<void> togglePlayPause() => _session.togglePlayPause();

  @override
  Future<void> seek(Duration position) => _session.seek(position);

  Future<void> setVolume(double volume) => _session.setVolume(volume);

  Future<void> setMuted(bool muted) => _session.setMuted(muted);

  @override
  Future<void> toggleMuted() => _session.toggleMuted();

  @override
  Future<void> setSpeed(double speed) => _session.setSpeed(speed);

  @override
  Future<void> setLooping(bool looping) => _session.setLooping(looping);

  Future<void> refreshTracks() => _session.refreshTracks();

  Future<void> selectTrack(MediaTrack track, {bool enable = true}) =>
      _session.selectTrack(track, enable: enable);

  Future<void> setVideoOrientation(VideoOrientationConfig config) =>
      _session.setVideoOrientation(config);

  @override
  Future<void> setAspectRatioMode(AspectRatioMode mode) =>
      _session.setAspectRatioMode(mode);

  Future<void> dispose() => _session.dispose();
}
