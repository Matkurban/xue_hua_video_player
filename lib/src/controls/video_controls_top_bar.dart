import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../theme/video_controls_theme.dart';
import '../utils/platform_util.dart';
import 'aspect_ratio_menu.dart';
import 'controls_overlay_slots.dart';
import 'fullscreen_config.dart';
import 'immersive_controls_state.dart';
import 'playback_controls_model.dart';
import 'video_orientation_menu_button.dart';

/// AppBar 式沉浸控件顶栏 / AppBar-style top bar for immersive video controls.
class VideoControlsTopBar extends StatelessWidget {
  /// 创建顶栏 / Creates the top bar.
  const VideoControlsTopBar({
    super.key,
    required this.immersive,
    required this.model,
    required this.theme,
    required this.slots,
    required this.labels,
    required this.orientationLabels,
    required this.showOrientationMenu,
  });

  /// 沉浸 signals / Immersive signals.
  final ImmersiveControlsState immersive;

  /// 播放控件 model / Playback controls model.
  final PlaybackControlsModel model;

  /// 控件主题 / Controls theme.
  final VideoControlsTheme theme;

  /// 顶栏插槽 / Top bar slots.
  final VideoControlsOverlaySlots slots;

  /// 铺满模式文案 / Aspect ratio mode labels.
  final AspectRatioModeLabels labels;

  /// 旋转面板文案 / Video rotation panel labels.
  final VideoRotationLabels orientationLabels;

  /// 是否在顶栏显示方向设置按钮 / Whether to show orientation button in the top bar.
  final bool showOrientationMenu;

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        final immersiveActive = immersive.immersiveActive.value;
        final hasCustomSlots =
            slots.leading != null ||
            slots.title != null ||
            slots.actions.isNotEmpty;

        if (!immersiveActive && !hasCustomSlots) {
          return const SizedBox.shrink();
        }

        final showOrientationButton =
            showOrientationMenu &&
            model.supportsOrientation.value &&
            (isMobilePlatform
                ? immersive.landscapeLocked.value
                : immersiveActive || hasCustomSlots);

        final actions = <Widget>[
          ...slots.actions,
          if (showOrientationButton)
            VideoOrientationMenuButton(
              model: model,
              theme: theme,
              labels: orientationLabels,
            ),
          if (slots.showAspectRatioMenu && immersiveActive)
            AspectRatioMenuButton(
              immersive: immersive,
              theme: theme,
              labels: labels,
            ),
        ];

        return Positioned(
          top: 0,
          left: 0,
          right: 0,
          child: SafeArea(
            bottom: false,
            child: Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Row(
                children: [
                  if (slots.leading != null) slots.leading!,
                  if (slots.title != null)
                    Expanded(child: slots.title!)
                  else if (slots.leading != null || actions.isNotEmpty)
                    const Spacer(),
                  if (actions.isNotEmpty)
                    Padding(
                      padding: slots.actionsPadding,
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: actions,
                      ),
                    ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}
