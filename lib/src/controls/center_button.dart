import 'package:flutter/cupertino.dart';
import 'package:liquid_glass_widgets/liquid_glass_widgets.dart';
import 'package:signals/signals_flutter.dart';

import '../rust/player_events.dart';
import '../theme/video_controls_theme.dart';
import 'playback_controls_model.dart';

/// 两种控件风格共用的中央播放/暂停/缓冲 affordance / Central play/pause/buffering affordance shared by both control styles.
///
/// 缓冲中隐藏大按钮（由进度条与 presentation 层指示）；否则显示玻璃态播放/暂停。
/// Hidden during buffering (progress/presentation indicate loading); otherwise shows glass play/pause.
class CenterButton extends StatelessWidget {
  const CenterButton({
    super.key,
    required this.model,
    required this.theme,
    required this.onInteract,
  });

  final PlaybackControlsModel model;
  final VideoControlsTheme theme;
  final VoidCallback onInteract;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: SignalBuilder(
        builder: (context) {
          final PlayerState state = model.state.value;
          final buffering = model.bufferingPercent.value;
          if (buffering < 100 || state == PlayerState.buffering) {
            return SizedBox(
              width: theme.primaryIconSize,
              height: theme.primaryIconSize,
            );
          }
          final playing = state == PlayerState.playing;
          final icon = (playing
              ? CupertinoIcons.pause_solid
              : CupertinoIcons.play_arrow_solid);
          return GlassIconButton(
            onPressed: () async {
              onInteract();
              await model.togglePlayPause();
            },
            icon: Icon(
              icon,
              size: theme.primaryIconSize,
              color: theme.iconColor,
            ),
          );
        },
      ),
    );
  }
}
