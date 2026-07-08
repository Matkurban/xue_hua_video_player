import 'package:signals/signals_flutter.dart';

import '../rust/player_events.dart';

/// 内置视频控件窄接口：只读 transport 状态 + 命令 / Narrow seam for built-in controls: readonly transport state and commands.
///
/// 由 [XueHuaPlayerController] 实现；[VideoControls] 及其子组件依赖此接口。
/// Implemented by [XueHuaPlayerController]; [VideoControls] and child widgets depend on it.
abstract class PlaybackControlsModel {
  ReadonlySignal<PlayerState> get state;
  ReadonlySignal<int> get bufferingPercent;
  ReadonlySignal<bool> get isPlaying;
  ReadonlySignal<Duration> get position;
  ReadonlySignal<Duration> get duration;
  ReadonlySignal<bool> get isSeekable;
  ReadonlySignal<bool> get muted;
  ReadonlySignal<double> get volume;
  ReadonlySignal<bool> get looping;
  ReadonlySignal<double> get speed;

  /// 当前 pipeline 是否支持视频方向变换 / Whether orientation transforms are supported.
  ReadonlySignal<bool> get supportsOrientation;

  /// 当前视频方向配置 / Current video orientation configuration.
  ReadonlySignal<VideoOrientationConfig> get videoOrientation;

  Future<void> togglePlayPause();
  Future<void> toggleMuted();
  Future<void> setLooping(bool looping);
  Future<void> setSpeed(double speed);
  Future<void> seek(Duration position);

  /// 设置音量 0.0–1.0 / Sets volume in 0.0–1.0.
  Future<void> setVolume(double volume);

  /// 设置铺满模式并同步至 pipeline / Sets aspect ratio mode and syncs to pipeline.
  Future<void> setAspectRatioMode(AspectRatioMode mode);

  /// 设置视频翻转/旋转（需 [supportsOrientation]）/ Sets flip/rotate when [supportsOrientation].
  Future<void> setVideoOrientation(VideoOrientationConfig config);
}
