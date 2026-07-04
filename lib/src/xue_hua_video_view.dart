import 'package:flutter/widgets.dart';
import 'package:signals/signals_flutter.dart';

import 'controls/video_controls.dart';
import 'enum/video_controls_style.dart';
import 'xue_hua_player_controller.dart';

export 'controls/video_controls.dart';

/// Renders the video frames produced by a [XueHuaPlayerController] into a
/// Flutter [Texture], preserving the decoded aspect ratio and optionally
/// overlaying a built-in adaptive control bar.
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

  /// Whether to overlay the built-in control bar. When `false`, only the raw
  /// video texture is shown and the widget renders no controls at all.
  final bool showControls;

  /// Which look the built-in controls adopt (adaptive by default).
  final VideoControlsStyle controlsStyle;

  @override
  Widget build(BuildContext context) {
    return ColoredBox(
      color: backgroundColor,
      child: Stack(
        fit: StackFit.expand,
        alignment: .center,
        children: [
          SignalBuilder(
            builder: (context) {
              final textureId = controller.textureId.value;
              if (textureId == null) return const SizedBox.expand();
              final size = controller.videoSize.value;
              return AspectRatio(
                aspectRatio: controller.aspectRatio.value,
                child: FittedBox(
                  fit: fit,
                  clipBehavior: Clip.hardEdge,
                  child: AnimatedContainer(
                    duration: const Duration(milliseconds: 200),
                    width: size.width > 0 ? size.width : 16,
                    height: size.height > 0 ? size.height : 9,
                    child: Texture(textureId: textureId),
                  ),
                ),
              );
            },
          ),
          if (showControls)
            VideoControls(controller: controller, style: controlsStyle),
        ],
      ),
    );
  }
}
