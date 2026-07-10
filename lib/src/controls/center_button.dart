import 'dart:ui';

import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../domain/player_events.dart';
import '../theme/video_controls_theme.dart';
import '../utils/platform_util.dart';
import 'immersive_controls_state.dart';
import 'playback_controls_model.dart';

/// 两种控件风格共用的中央播放/暂停/缓冲 affordance / Central play/pause/buffering affordance shared by both control styles.
///
/// 缓冲中隐藏大按钮（由进度条与 presentation 层指示）；否则显示磨砂圆形播放/暂停。
/// Hidden during buffering (progress/presentation indicate loading); otherwise shows a frosted play/pause.
class CenterButton extends StatelessWidget {
  const CenterButton({
    super.key,
    required this.model,
    required this.theme,
    required this.onInteract,
    this.hud,
  });

  final PlaybackControlsModel model;
  final VideoControlsTheme theme;
  final VoidCallback onInteract;

  /// 沉浸 HUD；非空时隐藏中央按钮避免与正中 HUD 重叠 / Hides button while center HUD is shown.
  final ReadonlySignal<ImmersiveHudSnapshot?>? hud;

  static bool _hideForHud(ImmersiveHudSnapshot snap) =>
      isMobilePlatform || snap.kind == ImmersiveHudKind.playPause;

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        final snap = hud?.value;
        if (snap != null && _hideForHud(snap)) {
          return SizedBox(
            width: theme.centerButtonSize,
            height: theme.centerButtonSize,
          );
        }
        final PlayerState state = model.state.value;
        final buffering = model.bufferingPercent.value;
        if (buffering < 100 || state == PlayerState.buffering) {
          return SizedBox(
            width: theme.centerButtonSize,
            height: theme.centerButtonSize,
          );
        }
        final playing = state == PlayerState.playing;
        final icon = playing
            ? CupertinoIcons.pause_solid
            : CupertinoIcons.play_arrow_solid;
        return SizedBox(
          width: theme.centerButtonSize,
          height: theme.centerButtonSize,
          child: ClipOval(
            child: BackdropFilter(
              filter: ImageFilter.blur(sigmaX: 8, sigmaY: 8),
              child: DecoratedBox(
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: theme.backgroundColor,
                ),
                child: IconButton(
                  onPressed: () async {
                    onInteract();
                    await model.togglePlayPause();
                  },
                  style: IconButton.styleFrom(
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    visualDensity: VisualDensity.compact,
                  ),
                  icon: Icon(
                    icon,
                    size: theme.primaryIconSize,
                    color: theme.iconColor,
                  ),
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}
