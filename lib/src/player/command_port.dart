import '../domain/player_events.dart';
import 'ffi_command_port.dart';

/// Dart/native 接缝：播放器完整生命周期与播放命令。
///
/// 抽象原生调用，便于测试注入 fake port。
abstract class PlayerCommandPort {
  /// 原生 player ID；[create] 后可用。
  int? get playerId;

  /// 创建原生 player。
  Future<void> create();

  /// 原生推送的 [PlayerEvent] 广播流。
  Stream<PlayerEvent> get events;

  /// 销毁 player 并释放资源。
  Future<void> dispose();

  /// 加载 [source]；[autoPlay] 为 true 时加载后立即播放。
  Future<void> loadSource(MediaSourceDto source, {required bool autoPlay});

  /// 查询当前 pipeline 能力（seek、多轨、方向）。
  Future<PipelineCapabilitiesDto> getPipelineCapabilities();

  /// 获取当前媒体轨道列表。
  Future<List<MediaTrack>> getTracks();

  /// 播放。
  Future<void> play();

  /// 暂停。
  Future<void> pause();

  /// 停止。
  Future<void> stop();

  /// 跳转。
  Future<void> seek(Duration position);

  /// 设置音量 0.0–1.0。
  Future<void> setVolume(double volume);

  /// 设置静音。
  Future<void> setMute(bool mute);

  /// 设置倍速。
  Future<void> setSpeed(double speed);

  /// 设置循环。
  Future<void> setLooping(bool looping);

  /// 选中或取消选中轨道。
  Future<void> selectTrack(MediaTrack track, {required bool enable});

  /// 设置视频顺时针旋转角度。
  Future<void> setVideoRotation(int rotateDegrees);

  /// 设置宽高比模式。
  Future<void> setAspectRatioMode(AspectRatioMode mode);

  /// 拷贝当前最新视频帧（BGRA map：bytes/width/height/stride）。
  Future<Map<String, Object?>> captureCurrentFrame();
}

/// 生产环境适配器（Dart FFI → C 播放器核心）。
typedef ProductionPlayerCommandPort = FfiPlayerCommandPort;
