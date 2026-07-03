import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../player_controller.dart';
import '../theme/video_controls_theme.dart';

/// Central play/pause/buffering affordance shared by both styles.
class CenterButton extends StatelessWidget {
  const CenterButton({
    super.key,
    required this.controller,
    required this.theme,
    required this.onInteract,
    required this.cupertino,
  });

  final XueHuaPlayerController controller;
  final VideoControlsTheme theme;
  final VoidCallback onInteract;
  final bool cupertino;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: SignalBuilder(
        builder: (context) {
          final state = controller.state.value;
          if (state == PlayerState.buffering) {
            return cupertino
                ? CupertinoActivityIndicator(
                    color: theme.iconColor,
                    radius: theme.primaryIconSize / 2.4,
                  )
                : SizedBox(
                    width: theme.primaryIconSize,
                    height: theme.primaryIconSize,
                    child: CircularProgressIndicator(
                      strokeWidth: 3,
                      valueColor: AlwaysStoppedAnimation<Color>(
                        theme.iconColor,
                      ),
                    ),
                  );
          }
          final playing = state == PlayerState.playing;
          final icon = cupertino
              ? (playing
                    ? CupertinoIcons.pause_solid
                    : CupertinoIcons.play_arrow_solid)
              : (playing ? Icons.pause_circle_filled : Icons.play_circle_fill);
          return IconButton(
            onPressed: () {
              onInteract();
              controller.togglePlayPause();
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
