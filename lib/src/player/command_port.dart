import '../rust/api/player.dart' as rust;
import '../rust/player_events.dart';

/// Dart/Rust 接缝：播放器完整生命周期与播放命令 / Dart/Rust seam: full player lifecycle and playback commands.
///
/// 抽象 FRB 调用，便于测试注入 fake port。
/// Abstracts FRB calls so tests can inject fake implementations.
abstract class PlayerCommandPort {
  /// 原生 player ID；[create] 后可用 / Native player id; available after [create].
  int? get playerId;

  /// 创建原生 player / Creates the native player.
  Future<void> create();

  /// Rust 推送的 [PlayerEvent] 广播流 / Broadcast stream of [PlayerEvent] from Rust.
  Stream<PlayerEvent> get events;

  /// 销毁 player 并释放资源 / Disposes the player and releases resources.
  Future<void> dispose();

  /// 加载 [source]；[autoPlay] 为 true 时加载后立即播放 / Loads [source]; plays immediately when [autoPlay] is true.
  Future<void> loadSource(MediaSourceDto source, {required bool autoPlay});

  /// 查询当前 pipeline 能力（seek、多轨、方向）/ Queries active pipeline capabilities.
  Future<PipelineCapabilitiesDto> getPipelineCapabilities();

  /// 获取当前媒体轨道列表 / Returns tracks for the current media.
  Future<List<MediaTrack>> getTracks();

  /// 播放 / Play.
  Future<void> play();

  /// 暂停 / Pause.
  Future<void> pause();

  /// 停止 / Stop.
  Future<void> stop();

  /// 跳转 / Seek.
  Future<void> seek(Duration position);

  /// 设置音量 0.0–1.0 / Sets volume 0.0–1.0.
  Future<void> setVolume(double volume);

  /// 设置静音 / Sets mute.
  Future<void> setMute(bool mute);

  /// 设置倍速 / Sets playback speed.
  Future<void> setSpeed(double speed);

  /// 设置循环 / Sets looping.
  Future<void> setLooping(bool looping);

  /// 选中或取消选中轨道 / Selects or deselects a track.
  Future<void> selectTrack(MediaTrack track, {required bool enable});

  /// 设置视频方向 / Sets video orientation.
  Future<void> setVideoOrientation(VideoOrientationConfig config);

  /// 设置宽高比模式 / Sets aspect ratio mode.
  Future<void> setAspectRatioMode(AspectRatioMode mode);
}

/// 生产环境 FRB 适配器 / Production adapter over FRB `rust/api/player.dart`.
///
/// [create] 分配 [playerId]；后续命令均携带该 ID 调用 Rust API。
/// [create] assigns [playerId]; subsequent commands pass that id to Rust APIs.
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
