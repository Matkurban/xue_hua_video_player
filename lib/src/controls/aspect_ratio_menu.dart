import 'package:chat_context_menu/chat_context_menu.dart';
import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../domain/player_events.dart';
import '../theme/video_controls_theme.dart';
import 'fullscreen_config.dart';
import 'immersive_controls_state.dart';

/// 铺满模式菜单触发按钮（无定位）/ Aspect ratio menu trigger without positioning.
class AspectRatioMenuButton extends StatelessWidget {
  /// 创建铺满模式按钮 / Creates the aspect ratio menu button.
  const AspectRatioMenuButton({
    super.key,
    required this.immersive,
    required this.theme,
    required this.labels,
  });

  /// 沉浸 signals / Immersive signals.
  final ImmersiveControlsState immersive;

  /// 控件主题 / Controls theme.
  final VideoControlsTheme theme;

  /// 选项文案 / Option labels.
  final AspectRatioModeLabels labels;

  @override
  Widget build(BuildContext context) {
    return ChatContextMenuWrapper(
      topPadding: 0,
      backgroundColor: theme.backgroundColor,
      borderRadius: BorderRadius.circular(theme.borderRadius),
      padding: EdgeInsets.zero,
      menuBuilder: (context, hideMenu) {
        return SignalBuilder(
          builder: (context) {
            final current = immersive.aspectRatioMode.value;
            return Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final mode in AspectRatioMode.values)
                  _ModeTile(
                    label: labels.label(mode),
                    selected: mode == current,
                    textColor: theme.textColor,
                    onTap: () {
                      immersive.aspectRatioMode.value = mode;
                      hideMenu();
                    },
                  ),
              ],
            );
          },
        );
      },
      widgetBuilder: (context, showMenu, _) {
        return SignalBuilder(
          builder: (context) {
            final label = labels.label(immersive.aspectRatioMode.value);
            return GestureDetector(
              onTap: showMenu,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: theme.backgroundColor,
                  borderRadius: BorderRadius.circular(theme.borderRadius),
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 6,
                  ),
                  child: Text(
                    label,
                    style: TextStyle(
                      color: theme.textColor,
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ),
            );
          },
        );
      },
    );
  }
}

class _ModeTile extends StatelessWidget {
  const _ModeTile({
    required this.label,
    required this.selected,
    required this.textColor,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final Color textColor;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            SizedBox(
              width: 20,
              child: selected
                  ? Icon(Icons.check, size: 16, color: textColor)
                  : null,
            ),
            Text(label, style: TextStyle(color: textColor, fontSize: 14)),
          ],
        ),
      ),
    );
  }
}
