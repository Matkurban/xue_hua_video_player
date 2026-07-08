import 'package:chat_context_menu/chat_context_menu.dart';
import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../rust/player_events.dart';
import '../theme/video_controls_theme.dart';
import 'fullscreen_config.dart';
import 'playback_controls_model.dart';

/// 视频方向设置按钮（无定位）/ Video orientation settings trigger without positioning.
class VideoOrientationMenuButton extends StatelessWidget {
  /// 创建方向设置按钮 / Creates the orientation settings button.
  const VideoOrientationMenuButton({
    super.key,
    required this.model,
    required this.theme,
    required this.labels,
  });

  /// 播放控件 model / Playback controls model.
  final PlaybackControlsModel model;

  /// 控件主题 / Controls theme.
  final VideoControlsTheme theme;

  /// 面板文案 / Panel labels.
  final VideoOrientationLabels labels;

  static const List<int> _rotateOptions = [0, 90, 180, 270];

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        if (!model.supportsOrientation.value) {
          return const SizedBox.shrink();
        }

        return ChatContextMenuWrapper(
          topPadding: 0,
          backgroundColor: theme.backgroundColor,
          borderRadius: BorderRadius.circular(theme.borderRadius),
          padding: EdgeInsets.zero,
          menuBuilder: (context, hideMenu) {
            return SignalBuilder(
              builder: (context) {
                final config = model.videoOrientation.value;
                return Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    _OrientationSwitchTile(
                      label: labels.flipHorizontal,
                      value: config.flipHorizontal,
                      textColor: theme.textColor,
                      onChanged: (value) async {
                        await model.setVideoOrientation(
                          VideoOrientationConfig(
                            flipHorizontal: value,
                            flipVertical: config.flipVertical,
                            rotateDegrees: config.rotateDegrees,
                          ),
                        );
                      },
                    ),
                    _OrientationSwitchTile(
                      label: labels.flipVertical,
                      value: config.flipVertical,
                      textColor: theme.textColor,
                      onChanged: (value) async {
                        await model.setVideoOrientation(
                          VideoOrientationConfig(
                            flipHorizontal: config.flipHorizontal,
                            flipVertical: value,
                            rotateDegrees: config.rotateDegrees,
                          ),
                        );
                      },
                    ),
                    for (final degrees in _rotateOptions)
                      _OrientationOptionTile(
                        label: labels.rotateLabel(degrees),
                        selected: config.rotateDegrees == degrees,
                        textColor: theme.textColor,
                        onTap: () async {
                          await model.setVideoOrientation(
                            VideoOrientationConfig(
                              flipHorizontal: config.flipHorizontal,
                              flipVertical: config.flipVertical,
                              rotateDegrees: degrees,
                            ),
                          );
                          hideMenu();
                        },
                      ),
                  ],
                );
              },
            );
          },
          widgetBuilder: (context, showMenu, _) {
            return GestureDetector(
              onTap: showMenu,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: theme.backgroundColor,
                  borderRadius: BorderRadius.circular(theme.borderRadius),
                ),
                child: Padding(
                  padding: const EdgeInsets.all(8),
                  child: Icon(
                    Icons.screen_rotation,
                    size: theme.secondaryIconSize,
                    color: theme.iconColor,
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

class _OrientationSwitchTile extends StatelessWidget {
  const _OrientationSwitchTile({
    required this.label,
    required this.value,
    required this.textColor,
    required this.onChanged,
  });

  final String label;
  final bool value;
  final Color textColor;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(label, style: TextStyle(color: textColor, fontSize: 14)),
          const SizedBox(width: 12),
          Switch(
            value: value,
            onChanged: onChanged,
            materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
          ),
        ],
      ),
    );
  }
}

class _OrientationOptionTile extends StatelessWidget {
  const _OrientationOptionTile({
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
