import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../rust/api/types.dart';
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

  /// Letterbox/crop/stretch behaviour. On Texture surfaces this is applied in
  /// Dart layout; on Android it is also forwarded to `glimagesink`.
  final AspectRatioMode aspectRatioMode;

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        final playerId = model.playerId.value;
        if (playerId == null) return const SizedBox.shrink();
        final handle = VideoSurfaceHandle.fromPlayerId(playerId);
        final ratio = model.aspectRatio.value;
        final surface = Stack(
          fit: StackFit.expand,
          children: [
            buildVideoSurface(handle),
            _BufferingOverlay(model: model),
          ],
        );
        return _AspectRatioModeSync(
          model: model,
          aspectRatioMode: aspectRatioMode,
          child: _VideoAspectLayout(
            aspectRatio: ratio,
            mode: aspectRatioMode,
            child: surface,
          ),
        );
      },
    );
  }
}

/// Sizes the video surface for external [Texture] rendering.
///
/// GStreamer `appsink` frames are raw rectangles; Flutter `Texture` stretches
/// pixels to the widget bounds, so the widget itself must preserve DAR.
class _VideoAspectLayout extends StatelessWidget {
  const _VideoAspectLayout({
    required this.aspectRatio,
    required this.mode,
    required this.child,
  });

  final double aspectRatio;
  final AspectRatioMode mode;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final ratio = aspectRatio > 0 ? aspectRatio : 16 / 9;

    switch (mode) {
      case AspectRatioMode.fit:
        return Center(
          child: AspectRatio(
            aspectRatio: ratio,
            child: child,
          ),
        );
      case AspectRatioMode.fill:
        return Center(
          child: AspectRatio(
            aspectRatio: ratio,
            child: FittedBox(
              fit: BoxFit.cover,
              alignment: Alignment.center,
              clipBehavior: Clip.hardEdge,
              child: SizedBox(
                width: ratio,
                height: 1,
                child: child,
              ),
            ),
          ),
        );
      case AspectRatioMode.stretch:
        return SizedBox.expand(child: child);
    }
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
