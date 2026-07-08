import 'package:chat_context_menu/chat_context_menu.dart';
import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../enum/video_rotation.dart';
import '../theme/video_controls_theme.dart';
import 'fullscreen_config.dart';
import 'playback_controls_model.dart';
import 'themed_segmented_control.dart';

/// 视频旋转设置按钮（无定位）/ Video rotation settings trigger without positioning.
class VideoOrientationMenuButton extends StatelessWidget {
  /// 创建旋转设置按钮 / Creates the rotation settings button.
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
  final VideoRotationLabels labels;

  static const List<VideoRotation> _rotateOptions = [
    VideoRotation.deg0,
    VideoRotation.deg90,
    VideoRotation.deg180,
    VideoRotation.deg270,
  ];

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
                final rotation = model.videoRotation.value;
                return SizedBox(
                  width: 280,
                  child: Padding(
                    padding: const EdgeInsets.all(12),
                    child: ThemedSegmentedControl<VideoRotation>(
                      label: labels.rotateAngle,
                      theme: theme,
                      showSelectedIcon: false,
                      segments: [
                        for (final option in _rotateOptions)
                          ButtonSegment<VideoRotation>(
                            value: option,
                            label: Text(labels.rotateLabel(option.degrees)),
                          ),
                      ],
                      selected: {rotation},
                      onSelectionChanged: (value) async {
                        await model.setVideoRotation(value.first);
                      },
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
