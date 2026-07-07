import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import 'controls/video_controls.dart';
import 'enum/video_controls_style.dart';
import 'surface/build_video_surface.dart';
import 'surface/video_surface_handle.dart';
import 'xue_hua_player_controller.dart';

export 'controls/video_controls.dart';

/// Renders GStreamer video into a native Platform View, preserving aspect ratio
/// and optionally overlaying a built-in adaptive control bar.
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

  /// How GStreamer scales video inside the platform sink (`force-aspect-ratio`).
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
            SignalBuilder(
              builder: (context) {
                final playerId = controller.playerId.value;
                if (playerId == null) return const SizedBox.expand();
                final handle = VideoSurfaceHandle.fromPlayerId(playerId);
                return _AspectRatioModeSync(
                  controller: controller,
                  aspectRatioMode: aspectRatioMode,
                  child: AspectRatio(
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
                              buildVideoSurface(handle),
                              SignalBuilder(
                                builder: (context) {
                                  final buffering =
                                      controller.bufferingPercent.value;
                                  final state = controller.state.value;
                                  final showLoading =
                                      state == PlayerState.buffering;
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
                  ),
                );
              },
            ),
            if (showControls)
              VideoControls(model: controller, style: controlsStyle),
          ],
        ),
      ),
    );
  }
}

/// Pushes [aspectRatioMode] to the Rust pipeline when the player is ready.
class _AspectRatioModeSync extends StatefulWidget {
  const _AspectRatioModeSync({
    required this.controller,
    required this.aspectRatioMode,
    required this.child,
  });

  final XueHuaPlayerController controller;
  final AspectRatioMode aspectRatioMode;
  final Widget child;

  @override
  State<_AspectRatioModeSync> createState() => _AspectRatioModeSyncState();
}

class _AspectRatioModeSyncState extends State<_AspectRatioModeSync> {
  @override
  void initState() {
    super.initState();
    _sync();
  }

  @override
  void didUpdateWidget(covariant _AspectRatioModeSync oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.aspectRatioMode != widget.aspectRatioMode ||
        oldWidget.controller != widget.controller) {
      _sync();
    }
  }

  void _sync() {
    if (widget.controller.playerId.value == null) return;
    if (!widget.controller.initialized.value) return;
    widget.controller.setAspectRatioMode(widget.aspectRatioMode);
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
