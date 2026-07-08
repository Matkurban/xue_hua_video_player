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

/// 视频方向面板的显示文案 / Display labels for the video orientation panel.
class VideoOrientationLabels {
  /// 创建方向面板文案 / Creates orientation panel labels.
  const VideoOrientationLabels({
    this.flipHorizontal = '水平翻转',
    this.flipVertical = '垂直翻转',
    this.flipOff = '关',
    this.flipOn = '开',
    this.rotateAngle = '旋转角度',
    this.rotate0 = '0°',
    this.rotate90 = '90°',
    this.rotate180 = '180°',
    this.rotate270 = '270°',
  });

  /// 水平翻转 / Horizontal flip.
  final String flipHorizontal;

  /// 垂直翻转 / Vertical flip.
  final String flipVertical;

  /// 翻转关闭 / Flip off label.
  final String flipOff;

  /// 翻转开启 / Flip on label.
  final String flipOn;

  /// 旋转角度区标题 / Rotation section label.
  final String rotateAngle;

  /// 顺时针 0° / Clockwise 0 degrees.
  final String rotate0;

  /// 顺时针 90° / Clockwise 90 degrees.
  final String rotate90;

  /// 顺时针 180° / Clockwise 180 degrees.
  final String rotate180;

  /// 顺时针 270° / Clockwise 270 degrees.
  final String rotate270;

  /// 返回 [degrees] 对应文案 / Returns label for [degrees].
  String rotateLabel(int degrees) => switch (degrees) {
    0 => rotate0,
    90 => rotate90,
    180 => rotate180,
    270 => rotate270,
    _ => '$degrees°',
  };

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is VideoOrientationLabels &&
          flipHorizontal == other.flipHorizontal &&
          flipVertical == other.flipVertical &&
          flipOff == other.flipOff &&
          flipOn == other.flipOn &&
          rotateAngle == other.rotateAngle &&
          rotate0 == other.rotate0 &&
          rotate90 == other.rotate90 &&
          rotate180 == other.rotate180 &&
          rotate270 == other.rotate270;

  @override
  int get hashCode => Object.hash(
    flipHorizontal,
    flipVertical,
    flipOff,
    flipOn,
    rotateAngle,
    rotate0,
    rotate90,
    rotate180,
    rotate270,
  );
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
  /// - `showOrientationMenu` — 是否显示方向设置按钮（移动端仅全屏；桌面端顶栏可见时）/ orientation button visibility
  /// - `orientationLabels` — 方向面板文案 / labels for orientation panel
  const VideoControlsFullscreenConfig({
    this.seekStep = const Duration(seconds: 5),
    this.desktopImmersive = true,
    this.aspectRatioLabels = const AspectRatioModeLabels(),
    this.overlaySlots = const VideoControlsOverlaySlots(),
    this.showOrientationMenu = true,
    this.orientationLabels = const VideoOrientationLabels(),
  });

  /// 单次进退时长 / Seek step per gesture or arrow key.
  final Duration seekStep;

  /// 桌面端沉浸能力开关（默认开启，无全屏按钮）/ Desktop immersive toggle (default on, no fullscreen button).
  final bool desktopImmersive;

  /// 铺满模式选项文案 / Labels for aspect ratio mode options.
  final AspectRatioModeLabels aspectRatioLabels;

  /// 沉浸顶栏插槽 / AppBar-style overlay slots for immersive controls.
  final VideoControlsOverlaySlots overlaySlots;

  /// 是否显示视频方向设置按钮；移动端仅全屏，桌面端在顶栏可见时显示 / Orientation button; mobile fullscreen only, desktop when top bar shows.
  final bool showOrientationMenu;

  /// 视频方向面板文案 / Labels for video orientation panel.
  final VideoOrientationLabels orientationLabels;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is VideoControlsFullscreenConfig &&
          seekStep == other.seekStep &&
          desktopImmersive == other.desktopImmersive &&
          aspectRatioLabels == other.aspectRatioLabels &&
          overlaySlots == other.overlaySlots &&
          showOrientationMenu == other.showOrientationMenu &&
          orientationLabels == other.orientationLabels;

  @override
  int get hashCode => Object.hash(
    seekStep,
    desktopImmersive,
    aspectRatioLabels,
    overlaySlots,
    showOrientationMenu,
    orientationLabels,
  );
}
