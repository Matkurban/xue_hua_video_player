import 'package:flutter/material.dart';

import 'controls/video_controls.dart';
import 'enum/video_controls_style.dart';
import 'presentation/playback_presentation.dart';
import 'xue_hua_player_controller.dart';

export 'controls/video_controls.dart';

/// Renders GStreamer video through a Flutter external texture, preserving aspect
/// ratio and optionally overlaying a built-in adaptive control bar.
class XueHuaVideoView extends StatelessWidget {
  const XueHuaVideoView({
    super.key,
    required this.controller,
    this.aspectRatioMode = AspectRatioMode.fit,
    this.backgroundColor = const Color(0xFF000000),
    this.showControls = true,
    this.controlsStyle = VideoControlsStyle.adaptive,
  });

  final XueHuaPlayerController controller;

  /// Letterbox / crop / stretch for the video surface. Applied in Dart for
  /// Texture rendering; also forwarded to GStreamer on Android (`glimagesink`).
  final AspectRatioMode aspectRatioMode;

  /// Painted behind/around the video (letterbox bars).
  final Color backgroundColor;

  /// Whether to overlay the built-in control bar.
  final bool showControls;

  /// Which look the built-in controls adopt (adaptive by default).
  final VideoControlsStyle controlsStyle;

  @override
  Widget build(BuildContext context) {
    return Material(
      type: MaterialType.transparency,
      child: ColoredBox(
        color: backgroundColor,
        child: Stack(
          fit: StackFit.expand,
          alignment: Alignment.center,
          children: [
            PlaybackPresentation(
              model: controller,
              aspectRatioMode: aspectRatioMode,
            ),
            if (showControls)
              VideoControls(model: controller, style: controlsStyle),
          ],
        ),
      ),
    );
  }
}
