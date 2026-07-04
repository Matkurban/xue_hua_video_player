import 'package:flutter/cupertino.dart';
import 'package:liquid_glass_widgets/liquid_glass_widgets.dart';
import 'package:signals/signals_flutter.dart';

import '../xue_hua_player_controller.dart';
import '../theme/video_controls_theme.dart';

/// Central play/pause/buffering affordance shared by both styles.
class CenterButton extends StatelessWidget {
  const CenterButton({
    super.key,
    required this.controller,
    required this.theme,
    required this.onInteract,
  });

  final XueHuaPlayerController controller;
  final VideoControlsTheme theme;
  final VoidCallback onInteract;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: SignalBuilder(
        builder: (context) {
          final PlayerState state = controller.state.value;
          if (state == PlayerState.buffering) {
            return CupertinoActivityIndicator(
              color: theme.iconColor,
              radius: theme.primaryIconSize / 2.4,
            );
          }
          final playing = state == PlayerState.playing;
          final icon = (playing
              ? CupertinoIcons.pause_solid
              : CupertinoIcons.play_arrow_solid);
          return GlassIconButton(
            onPressed: () async {
              onInteract();
              await controller.togglePlayPause();
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
