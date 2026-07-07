import 'dart:async';

import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../enum/video_controls_style.dart';
import '../rust/player_events.dart';
import '../theme/video_controls_theme.dart';
import 'cupertino_video_controls.dart';
import 'material_video_controls.dart';
import 'playback_controls_model.dart';

/// Overlay that draws an adaptive, auto-hiding control bar on top of the video.
///
/// Tap the video to toggle the controls; they hide automatically after a few
/// seconds of inactivity while playing. All reactive reads happen inside
/// [SignalBuilder]s so only the affected control rebuilds.
class VideoControls extends StatefulWidget {
  const VideoControls({
    super.key,
    required this.model,
    this.style = VideoControlsStyle.adaptive,
    this.autoHide = const Duration(seconds: 3),
  });

  final PlaybackControlsModel model;
  final VideoControlsStyle style;
  final Duration autoHide;

  @override
  State<VideoControls> createState() => _VideoControlsState();
}

class _VideoControlsState extends State<VideoControls> {
  final FlutterSignal<bool> _visible = signal(true);

  Timer? _hideTimer;
  late final void Function() _disposeStateEffect;

  @override
  void initState() {
    super.initState();
    _scheduleHide();
    _disposeStateEffect = effect(() {
      final state = widget.model.state.value;
      if (state == PlayerState.buffering || state == PlayerState.idle) {
        _visible.value = true;
        _scheduleHide();
      }
    });
  }

  @override
  void dispose() {
    _hideTimer?.cancel();
    _disposeStateEffect();
    super.dispose();
  }

  void _scheduleHide() {
    _hideTimer?.cancel();
    _hideTimer = Timer(widget.autoHide, () {
      if (!mounted) return;
      if (widget.model.isPlaying.value) {
        _visible.value = false;
      }
    });
  }

  void _keepAlive() {
    if (!_visible.value) {
      _visible.value = true;
    }
    _scheduleHide();
  }

  void _toggle() {
    _visible.value = !_visible.value;
    if (_visible.value) _scheduleHide();
  }

  bool _useCupertino(BuildContext context) {
    switch (widget.style) {
      case VideoControlsStyle.material:
        return false;
      case VideoControlsStyle.cupertino:
        return true;
      case VideoControlsStyle.adaptive:
        final platform = Theme.of(context).platform;
        return platform == TargetPlatform.iOS ||
            platform == TargetPlatform.macOS;
    }
  }

  @override
  Widget build(BuildContext context) {
    final cupertino = _useCupertino(context);
    final theme =
        Theme.of(context).extension<VideoControlsTheme>() ??
        (cupertino
            ? VideoControlsTheme.cupertino()
            : VideoControlsTheme.material());

    final controls = cupertino
        ? CupertinoVideoControls(
            model: widget.model,
            theme: theme,
            onInteract: _keepAlive,
          )
        : MaterialVideoControls(
            model: widget.model,
            theme: theme,
            onInteract: _keepAlive,
          );

    return Positioned.fill(
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: _toggle,
        child: SignalBuilder(
          builder: (context) {
            return AnimatedOpacity(
              opacity: _visible.value ? 1 : 0,
              duration: const Duration(milliseconds: 200),
              child: IgnorePointer(ignoring: !_visible.value, child: controls),
            );
          },
        ),
      ),
    );
  }
}
