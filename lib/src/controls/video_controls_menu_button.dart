import 'package:chat_context_menu/chat_context_menu.dart';
import 'package:flutter/material.dart';

import '../theme/video_controls_theme.dart';

/// 通用顶部栏菜单按钮 / Generic top bar menu button.
class VideoControlsMenuButton extends StatelessWidget {
  /// 创建通用菜单按钮 / Creates the generic menu button.
  const VideoControlsMenuButton({
    super.key,
    required this.theme,
    required this.icon,
    required this.menuBuilder,
  });

  /// 控件主题 / Controls theme.
  final VideoControlsTheme theme;

  /// 显示的图标 / Displayed icon.
  final IconData icon;

  /// 菜单内容构造器 / Context menu builder.
  final Widget Function(BuildContext context, VoidCallback hideMenu)
  menuBuilder;

  @override
  Widget build(BuildContext context) {
    return ChatContextMenuWrapper(
      topPadding: 0,
      backgroundColor: theme.backgroundColor,
      borderRadius: BorderRadius.circular(theme.borderRadius),
      padding: EdgeInsets.zero,
      menuBuilder: menuBuilder,
      widgetBuilder: (context, showMenu, _) {
        return GestureDetector(
          onTap: showMenu,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: theme.backgroundColor,
              borderRadius: BorderRadius.circular(theme.borderRadius),
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
              child: Icon(
                icon,
                size: theme.secondaryIconSize,
                color: theme.iconColor,
              ),
            ),
          ),
        );
      },
    );
  }
}
