import '../rust/player_events.dart';
import 'controls_overlay_slots.dart';

/// [AspectRatioMode] 的显示文案 / Display labels for each [AspectRatioMode].
class AspectRatioModeLabels {
  /// 创建铺满模式文案映射 / Creates label mapping for aspect ratio modes.
  const AspectRatioModeLabels({
    this.fit = '适应',
    this.fill = '铺满',
    this.stretch = '拉伸',
  });

  /// [AspectRatioMode.fit] 文案 / Label for letterbox fit.
  final String fit;

  /// [AspectRatioMode.fill] 文案 / Label for crop-to-fill.
  final String fill;

  /// [AspectRatioMode.stretch] 文案 / Label for stretch.
  final String stretch;

  /// 返回 [mode] 对应文案 / Returns the label for [mode].
  String label(AspectRatioMode mode) => switch (mode) {
    AspectRatioMode.fit => fit,
    AspectRatioMode.fill => fill,
    AspectRatioMode.stretch => stretch,
  };

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is AspectRatioModeLabels &&
          fit == other.fit &&
          fill == other.fill &&
          stretch == other.stretch;

  @override
  int get hashCode => Object.hash(fit, fill, stretch);
}

/// 全屏沉浸控件配置 / Configuration for fullscreen immersive controls.
class VideoControlsFullscreenConfig {
  /// 创建沉浸控件配置 / Creates immersive controls configuration.
  ///
  /// # 参数 / Parameters
  /// - `seekStep` — 手势/快捷键单次进退时长 / seek delta per gesture or key press
  /// - `desktopImmersive` — 桌面端是否默认开启沉浸能力 / immersive features on desktop
  /// - `aspectRatioLabels` — 铺满模式菜单文案 / labels for aspect ratio menu
  /// - `overlaySlots` — 沉浸顶栏 leading/title/actions 插槽 / top bar slots
  const VideoControlsFullscreenConfig({
    this.seekStep = const Duration(seconds: 5),
    this.desktopImmersive = true,
    this.aspectRatioLabels = const AspectRatioModeLabels(),
    this.overlaySlots = const VideoControlsOverlaySlots(),
  });

  /// 单次进退时长 / Seek step per gesture or arrow key.
  final Duration seekStep;

  /// 桌面端沉浸能力开关（默认开启，无全屏按钮）/ Desktop immersive toggle (default on, no fullscreen button).
  final bool desktopImmersive;

  /// 铺满模式选项文案 / Labels for aspect ratio mode options.
  final AspectRatioModeLabels aspectRatioLabels;

  /// 沉浸顶栏插槽 / AppBar-style overlay slots for immersive controls.
  final VideoControlsOverlaySlots overlaySlots;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is VideoControlsFullscreenConfig &&
          seekStep == other.seekStep &&
          desktopImmersive == other.desktopImmersive &&
          aspectRatioLabels == other.aspectRatioLabels &&
          overlaySlots == other.overlaySlots;

  @override
  int get hashCode =>
      Object.hash(seekStep, desktopImmersive, aspectRatioLabels, overlaySlots);
}
