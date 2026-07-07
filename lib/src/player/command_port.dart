import '../rust/api/player.dart' as rust;
import '../rust/player_events.dart';

/// Dart/Rust seam: full player lifecycle and playback commands.
abstract class PlayerCommandPort {
  int? get playerId;

  Future<void> create();

  Stream<PlayerEvent> get events;

  Future<void> dispose();

  Future<void> loadSource(MediaSourceDto source, {required bool autoPlay});

  Future<PipelineCapabilitiesDto> getPipelineCapabilities();

  Future<List<MediaTrack>> getTracks();

  Future<void> play();

  Future<void> pause();

  Future<void> stop();

  Future<void> seek(Duration position);

  Future<void> setVolume(double volume);

  Future<void> setMute(bool mute);

  Future<void> setSpeed(double speed);

  Future<void> setLooping(bool looping);

  Future<void> selectTrack(MediaTrack track, {required bool enable});

  Future<void> setVideoOrientation(VideoOrientationConfig config);

  Future<void> setAspectRatioMode(AspectRatioMode mode);
}

/// Production adapter over FRB `rust/api/player.dart`.
class ProductionPlayerCommandPort implements PlayerCommandPort {
  int? _playerId;

  @override
  int? get playerId => _playerId;

  int get _id {
    final id = _playerId;
    if (id == null) {
      throw StateError('PlayerCommandPort used before create()');
    }
    return id;
  }

  @override
  Future<void> create() async {
    final handle = await rust.createPlayer();
    _playerId = handle.playerId;
  }

  @override
  Stream<PlayerEvent> get events => rust.playerEventStream(playerId: _id);

  @override
  Future<void> dispose() async {
    final id = _playerId;
    _playerId = null;
    if (id != null) {
      await rust.disposePlayer(playerId: id);
    }
  }

  @override
  Future<void> loadSource(MediaSourceDto source, {required bool autoPlay}) =>
      rust.playerLoadSource(playerId: _id, source: source, autoPlay: autoPlay);

  @override
  Future<PipelineCapabilitiesDto> getPipelineCapabilities() =>
      rust.playerGetPipelineCapabilities(playerId: _id);

  @override
  Future<List<MediaTrack>> getTracks() => rust.playerGetTracks(playerId: _id);

  @override
  Future<void> play() => rust.playerPlay(playerId: _id);

  @override
  Future<void> pause() => rust.playerPause(playerId: _id);

  @override
  Future<void> stop() => rust.playerStop(playerId: _id);

  @override
  Future<void> seek(Duration position) =>
      rust.playerSeek(playerId: _id, positionMs: position.inMilliseconds);

  @override
  Future<void> setVolume(double volume) =>
      rust.playerSetVolume(playerId: _id, volume: volume);

  @override
  Future<void> setMute(bool mute) =>
      rust.playerSetMute(playerId: _id, mute: mute);

  @override
  Future<void> setSpeed(double speed) =>
      rust.playerSetSpeed(playerId: _id, speed: speed);

  @override
  Future<void> setLooping(bool looping) =>
      rust.playerSetLooping(playerId: _id, looping: looping);

  @override
  Future<void> selectTrack(MediaTrack track, {required bool enable}) =>
      rust.playerSelectTrack(
        playerId: _id,
        trackId: track.id,
        trackType: track.trackType,
        enable: enable,
      );

  @override
  Future<void> setVideoOrientation(VideoOrientationConfig config) =>
      rust.playerSetVideoOrientation(playerId: _id, config: config);

  @override
  Future<void> setAspectRatioMode(AspectRatioMode mode) =>
      rust.playerSetAspectRatioMode(playerId: _id, mode: mode);
}
