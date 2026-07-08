import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../rust/player_events.dart';
import '../surface/build_video_surface.dart';
import '../surface/video_surface_handle.dart';
import 'playback_presentation_model.dart';

/// Deep presentation module: platform surface embed, aspect layout, buffering chrome.
class PlaybackPresentation extends StatelessWidget {
  const PlaybackPresentation({
    super.key,
    required this.model,
    required this.aspectRatioMode,
  });

  final PlaybackPresentationModel model;

  /// How GStreamer scales video inside the platform sink (`force-aspect-ratio`).
  final AspectRatioMode aspectRatioMode;

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        final playerId = model.playerId.value;
        if (playerId == null) return const SizedBox.expand();
        final handle = VideoSurfaceHandle.fromPlayerId(playerId);
        return _AspectRatioModeSync(
          model: model,
          aspectRatioMode: aspectRatioMode,
          child: AspectRatio(
            aspectRatio: model.aspectRatio.value,
            child: LayoutBuilder(
              builder: (context, constraints) {
                if (constraints.maxWidth <= 0 || constraints.maxHeight <= 0) {
                  return const SizedBox.expand();
                }
                return Stack(
                  fit: StackFit.expand,
                  children: [
                    buildVideoSurface(handle),
                    _BufferingOverlay(model: model),
                  ],
                );
              },
            ),
          ),
        );
      },
    );
  }
}

class _BufferingOverlay extends StatelessWidget {
  const _BufferingOverlay({required this.model});

  final PlaybackPresentationModel model;

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        final buffering = model.bufferingPercent.value;
        final state = model.state.value;
        if (buffering >= 100 && state != PlayerState.buffering) {
          return const SizedBox.shrink();
        }
        return ColoredBox(
          color: Colors.black38,
          child: Center(
            child: CircularProgressIndicator(
              value: buffering < 100 ? buffering / 100 : null,
            ),
          ),
        );
      },
    );
  }
}

/// Pushes [aspectRatioMode] to the Rust pipeline when the player is ready.
class _AspectRatioModeSync extends StatefulWidget {
  const _AspectRatioModeSync({
    required this.model,
    required this.aspectRatioMode,
    required this.child,
  });

  final PlaybackPresentationModel model;
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
        oldWidget.model != widget.model) {
      _sync();
    }
  }

  void _sync() {
    if (widget.model.playerId.value == null) return;
    if (!widget.model.initialized.value) return;
    widget.model.setAspectRatioMode(widget.aspectRatioMode);
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
