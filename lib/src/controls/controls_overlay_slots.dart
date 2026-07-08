import 'package:flutter/material.dart';

/// 沉浸控件顶部栏插槽 / AppBar-style slots for the immersive controls top bar.
class VideoControlsOverlaySlots {
  /// 创建顶部栏插槽配置 / Creates top bar slot configuration.
  const VideoControlsOverlaySlots({
    this.leading,
    this.title,
    this.actions = const [],
    this.actionsPadding = const EdgeInsets.only(right: 8),
    this.showAspectRatioMenu = true,
  });

  /// 左侧 leading 插槽 / Leading slot (e.g. back button).
  final Widget? leading;

  /// 标题插槽；为 null 时不占位 / Title slot; null takes no space.
  final Widget? title;

  /// 右侧 actions 插槽 / Trailing action widgets.
  final List<Widget> actions;

  /// actions 区域额外内边距 / Extra padding around actions.
  final EdgeInsetsGeometry actionsPadding;

  /// 是否在 actions 末尾追加铺满模式按钮 / Append aspect ratio menu to actions.
  final bool showAspectRatioMenu;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is VideoControlsOverlaySlots &&
          leading == other.leading &&
          title == other.title &&
          _listEquals(actions, other.actions) &&
          actionsPadding == other.actionsPadding &&
          showAspectRatioMenu == other.showAspectRatioMenu;

  @override
  int get hashCode => Object.hash(
    leading,
    title,
    Object.hashAll(actions),
    actionsPadding,
    showAspectRatioMenu,
  );

  static bool _listEquals(List<Widget> a, List<Widget> b) {
    if (identical(a, b)) return true;
    if (a.length != b.length) return false;
    for (var i = 0; i < a.length; i++) {
      if (a[i] != b[i]) return false;
    }
    return true;
  }
}
