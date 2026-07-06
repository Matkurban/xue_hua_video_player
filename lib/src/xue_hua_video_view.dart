import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import 'controls/video_controls.dart';
import 'enum/video_controls_style.dart';
import 'platform_view.dart';
import 'xue_hua_player_controller.dart';

export 'controls/video_controls.dart';

/// Renders GStreamer video into a native Platform View, preserving aspect ratio
/// and optionally overlaying a built-in adaptive control bar.
class XueHuaVideoView extends StatelessWidget {
  const XueHuaVideoView({
    super.key,
    required this.controller,
    this.fit = BoxFit.contain,
    this.backgroundColor = const Color(0xFF000000),
    this.showControls = true,
    this.controlsStyle = VideoControlsStyle.adaptive,
  });

  final XueHuaPlayerController controller;

  /// How the video should be inscribed into the available space.
  final BoxFit fit;

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
            SignalBuilder(
              builder: (context) {
                final playerId = controller.playerId.value;
                if (playerId == null) return const SizedBox.expand();
                return AspectRatio(
                  aspectRatio: controller.aspectRatio.value,
                  child: ClipRect(
                    child: LayoutBuilder(
                      builder: (context, constraints) {
                        if (constraints.maxWidth <= 0 ||
                            constraints.maxHeight <= 0) {
                          return const SizedBox.expand();
                        }
                        return Stack(
                          fit: StackFit.expand,
                          children: [
                            buildXueHuaVideoPlatformView(playerId: playerId),
                            SignalBuilder(
                              builder: (context) {
                                final videoSize = controller.videoSize.value;
                                if (videoSize.width <= 0 || videoSize.height <= 0) {
                                  return const ColoredBox(color: Colors.black);
                                }
                                return const SizedBox.shrink();
                              },
                            ),
                            SignalBuilder(
                              builder: (context) {
                                final buffering =
                                    controller.bufferingPercent.value;
                                final state = controller.state.value;
                                final showLoading =
                                    state == PlayerState.buffering ||
                                    (buffering < 100 &&
                                        controller.isPlaying.value);
                                if (!showLoading) {
                                  return const SizedBox.shrink();
                                }
                                return ColoredBox(
                                  color: Colors.black38,
                                  child: Center(
                                    child: CircularProgressIndicator(
                                      value: buffering < 100
                                          ? buffering / 100
                                          : null,
                                    ),
                                  ),
                                );
                              },
                            ),
                          ],
                        );
                      },
                    ),
                  ),
                );
              },
            ),
            if (showControls)
              VideoControls(controller: controller, style: controlsStyle),
          ],
        ),
      ),
    );
  }
}
