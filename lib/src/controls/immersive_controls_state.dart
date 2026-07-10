import 'dart:async';

import 'package:signals/signals_flutter.dart';

import '../domain/player_events.dart';
import '../utils/platform_util.dart';
import 'fullscreen_config.dart';

/// 沉浸 HUD 反馈类型 / Kind of transient immersive HUD feedback.
enum ImmersiveHudKind {
  /// 进退预览 / Seek preview.
  seek,

  /// 亮度调节 / Brightness adjustment.
  brightness,

  /// 音量调节 / Volume adjustment.
  volume,

  /// 播放/暂停 / Play or pause toggle.
  playPause,
}

/// 沉浸 HUD 瞬时快照 / Transient immersive HUD snapshot.
class ImmersiveHudSnapshot {
  /// 创建 HUD 快照 / Creates a HUD snapshot.
  const ImmersiveHudSnapshot({
    required this.kind,
    required this.value,
    this.forward = true,
    this.gesture = false,
  });

  /// 反馈类型 / Feedback kind.
  final ImmersiveHudKind kind;

  /// 数值：秒数偏移、亮度或音量 0–1 / Value: seek seconds, brightness, or volume.
  final double value;

  /// 进退方向；仅 [ImmersiveHudKind.seek] 使用 / Seek direction; seek only.
  final bool forward;

  /// 是否是手势滑动触发 / Whether triggered by gesture drag.
  final bool gesture;
}

/// 沉浸控件 signals 单一数据源 / Single source of truth for immersive control signals.
///
/// 由 [XueHuaVideoView] 创建并 [dispose]；[VideoControls] 与子组件读取/写入。
/// Created and disposed by [XueHuaVideoView]; read/written by [VideoControls] and children.
class ImmersiveControlsState {
  /// 创建沉浸状态 / Creates immersive state.
  ImmersiveControlsState({
    required AspectRatioMode initialAspectRatioMode,
    required VideoControlsFullscreenConfig fullscreen,
  }) {
    landscapeLocked = signal(false);
    aspectRatioMode = signal(initialAspectRatioMode);
    hud = signal<ImmersiveHudSnapshot?>(null);
    this.fullscreen = signal(fullscreen);
    immersiveActive = computed(
      () => isMobilePlatform
          ? landscapeLocked.value
          : this.fullscreen.value.desktopImmersive,
    );
  }

  /// 沉浸配置 / Immersive configuration (updatable at runtime).
  late final FlutterSignal<VideoControlsFullscreenConfig> fullscreen;

  /// 移动端横屏锁定 / Mobile landscape lock (fullscreen).
  late final FlutterSignal<bool> landscapeLocked;

  /// 当前铺满模式 / Current aspect ratio mode.
  late final FlutterSignal<AspectRatioMode> aspectRatioMode;

  /// 瞬时 HUD；`null` 为隐藏 / Transient HUD; null when hidden.
  late final FlutterSignal<ImmersiveHudSnapshot?> hud;

  /// 沉浸能力是否激活 / Whether immersive features are active.
  late final ReadonlySignal<bool> immersiveActive;

  Timer? _hudTimer;

  /// 显示 HUD 并在 1 秒后自动隐藏 / Shows HUD and auto-hides after 1 second.
  void showHud(ImmersiveHudSnapshot snap) {
    hud.value = snap;
    _hudTimer?.cancel();
    _hudTimer = Timer(const Duration(seconds: 1), () {
      hud.value = null;
    });
  }

  /// 释放全部 signals / Disposes all signals.
  void dispose() {
    _hudTimer?.cancel();
    immersiveActive.dispose();
    fullscreen.dispose();
    hud.dispose();
    aspectRatioMode.dispose();
    landscapeLocked.dispose();
  }
}
