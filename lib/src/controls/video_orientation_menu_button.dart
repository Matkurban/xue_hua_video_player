import 'package:chat_context_menu/chat_context_menu.dart';
import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../rust/player_events.dart';
import '../theme/video_controls_theme.dart';
import 'fullscreen_config.dart';
import 'playback_controls_model.dart';
import 'themed_segmented_control.dart';

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
                return SizedBox(
                  width: 280,
                  child: Padding(
                    padding: const EdgeInsets.all(12),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        ThemedSegmentedControl<bool>(
                          label: labels.flipHorizontal,
                          theme: theme,
                          showSelectedIcon: false,
                          segments: [
                            ButtonSegment<bool>(
                              value: false,
                              label: Text(labels.flipOff),
                            ),
                            ButtonSegment<bool>(
                              value: true,
                              label: Text(labels.flipOn),
                            ),
                          ],
                          selected: {config.flipHorizontal},
                          onSelectionChanged: (value) async {
                            await model.setVideoOrientation(
                              VideoOrientationConfig(
                                flipHorizontal: value.first,
                                flipVertical: config.flipVertical,
                                rotateDegrees: config.rotateDegrees,
                              ),
                            );
                          },
                        ),
                        const SizedBox(height: 12),
                        ThemedSegmentedControl<bool>(
                          label: labels.flipVertical,
                          theme: theme,
                          showSelectedIcon: false,
                          segments: [
                            ButtonSegment<bool>(
                              value: false,
                              label: Text(labels.flipOff),
                            ),
                            ButtonSegment<bool>(
                              value: true,
                              label: Text(labels.flipOn),
                            ),
                          ],
                          selected: {config.flipVertical},
                          onSelectionChanged: (value) async {
                            await model.setVideoOrientation(
                              VideoOrientationConfig(
                                flipHorizontal: config.flipHorizontal,
                                flipVertical: value.first,
                                rotateDegrees: config.rotateDegrees,
                              ),
                            );
                          },
                        ),
                        const SizedBox(height: 12),
                        ThemedSegmentedControl<int>(
                          label: labels.rotateAngle,
                          theme: theme,
                          showSelectedIcon: false,
                          segments: [
                            for (final degrees in _rotateOptions)
                              ButtonSegment<int>(
                                value: degrees,
                                label: Text(labels.rotateLabel(degrees)),
                              ),
                          ],
                          selected: {config.rotateDegrees},
                          onSelectionChanged: (value) async {
                            await model.setVideoOrientation(
                              VideoOrientationConfig(
                                flipHorizontal: config.flipHorizontal,
                                flipVertical: config.flipVertical,
                                rotateDegrees: value.first,
                              ),
                            );
                          },
                        ),
                      ],
                    ),
                  ),
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
