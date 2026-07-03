import 'dart:async';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:liquid_glass_easy/liquid_glass_easy.dart';
import 'package:signals/signals_flutter.dart';

import '../player_controller.dart';
import '../theme/video_controls_theme.dart';

/// Selects which visual language the built-in [VideoControls] use.
enum VideoControlsStyle {
  /// Pick Cupertino on iOS/macOS, Material elsewhere.
  adaptive,

  /// Always use the Material control bar.
  material,

  /// Always use the Cupertino control bar.
  cupertino,
}

const List<double> _speeds = <double>[0.5, 1.0, 1.25, 1.5, 2.0];

String _formatDuration(Duration d) {
  final neg = d.isNegative;
  d = d.abs();
  final h = d.inHours;
  final m = d.inMinutes.remainder(60);
  final s = d.inSeconds.remainder(60);
  final mm = m.toString().padLeft(2, '0');
  final ss = s.toString().padLeft(2, '0');
  final body = h > 0 ? '$h:$mm:$ss' : '$mm:$ss';
  return neg ? '-$body' : body;
}

/// Overlay that draws an adaptive, auto-hiding control bar on top of the video.
///
/// Tap the video to toggle the controls; they hide automatically after a few
/// seconds of inactivity while playing. All reactive reads happen inside
/// [SignalBuilder]s so only the affected control rebuilds.
class VideoControls extends StatefulWidget {
  const VideoControls({
    super.key,
    required this.controller,
    this.style = VideoControlsStyle.adaptive,
    this.autoHide = const Duration(seconds: 3),
  });

  final XueHuaPlayerController controller;
  final VideoControlsStyle style;
  final Duration autoHide;

  @override
  State<VideoControls> createState() => _VideoControlsState();
}

class _VideoControlsState extends State<VideoControls> {
  bool _visible = true;
  Timer? _hideTimer;

  @override
  void initState() {
    super.initState();
    _scheduleHide();
  }

  @override
  void dispose() {
    _hideTimer?.cancel();
    super.dispose();
  }

  void _scheduleHide() {
    _hideTimer?.cancel();
    _hideTimer = Timer(widget.autoHide, () {
      if (!mounted) return;
      if (widget.controller.isPlaying.value) {
        setState(() => _visible = false);
      }
    });
  }

  void _keepAlive() {
    if (!_visible) {
      setState(() => _visible = true);
    }
    _scheduleHide();
  }

  void _toggle() {
    setState(() => _visible = !_visible);
    if (_visible) _scheduleHide();
  }

  bool _useCupertino(BuildContext context) {
    switch (widget.style) {
      case VideoControlsStyle.material:
        return false;
      case VideoControlsStyle.cupertino:
        return true;
      case VideoControlsStyle.adaptive:
        final platform = Theme.of(context).platform;
        return platform == TargetPlatform.iOS || platform == TargetPlatform.macOS;
    }
  }

  @override
  Widget build(BuildContext context) {
    final cupertino = _useCupertino(context);
    final theme =
        Theme.of(context).extension<VideoControlsTheme>() ??
        (cupertino ? VideoControlsTheme.cupertino() : VideoControlsTheme.material());

    final controls = cupertino
        ? _CupertinoVideoControls(
            controller: widget.controller,
            theme: theme,
            onInteract: _keepAlive,
          )
        : _MaterialVideoControls(
            controller: widget.controller,
            theme: theme,
            onInteract: _keepAlive,
          );

    return Positioned.fill(
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: _toggle,
        child: AnimatedOpacity(
          opacity: _visible ? 1 : 0,
          duration: const Duration(milliseconds: 200),
          child: IgnorePointer(ignoring: !_visible, child: controls),
        ),
      ),
    );
  }
}

/// Central play/pause/buffering affordance shared by both styles.
class _CenterButton extends StatelessWidget {
  const _CenterButton({
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
                      valueColor: AlwaysStoppedAnimation<Color>(theme.iconColor),
                    ),
                  );
          }
          final playing = state == PlayerState.playing;
          final icon = cupertino
              ? (playing ? CupertinoIcons.pause_solid : CupertinoIcons.play_arrow_solid)
              : (playing ? Icons.pause_circle_filled : Icons.play_circle_fill);
          return GestureDetector(
            onTap: () {
              onInteract();
              controller.togglePlayPause();
            },
            child: Icon(icon, size: theme.primaryIconSize, color: theme.iconColor),
          );
        },
      ),
    );
  }
}

class _MaterialVideoControls extends StatefulWidget {
  const _MaterialVideoControls({
    required this.controller,
    required this.theme,
    required this.onInteract,
  });

  final XueHuaPlayerController controller;
  final VideoControlsTheme theme;
  final VoidCallback onInteract;

  @override
  State<_MaterialVideoControls> createState() => _MaterialVideoControlsState();
}

class _MaterialVideoControlsState extends State<_MaterialVideoControls> {
  final FlutterSignal<double?> _dragValue = signal(null);

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final theme = widget.theme;
    return Stack(
      children: [
        _CenterButton(
          controller: controller,
          theme: theme,
          onInteract: widget.onInteract,
          cupertino: false,
        ),
        Positioned(
          left: 0,
          right: 0,
          bottom: 0,
          child: Container(
            padding: theme.barPadding,
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.bottomCenter,
                end: Alignment.topCenter,
                colors: [theme.backgroundColor, theme.backgroundColor.withValues(alpha: 0)],
              ),
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                SliderTheme(
                  data: SliderTheme.of(context).copyWith(
                    activeTrackColor: theme.activeTrackColor,
                    inactiveTrackColor: theme.inactiveTrackColor,
                    thumbColor: theme.thumbColor,
                    secondaryActiveTrackColor: theme.bufferedTrackColor,
                    trackHeight: 3,
                    overlayShape: const RoundSliderOverlayShape(overlayRadius: 12),
                    thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6),
                  ),
                  child: SignalBuilder(
                    builder: (context) {
                      final dur = controller.duration.value.inMilliseconds.toDouble();
                      final pos = controller.position.value.inMilliseconds.toDouble();
                      final value =
                          _dragValue.value ?? (dur > 0 ? (pos / dur).clamp(0.0, 1.0) : 0.0);
                      return TweenAnimationBuilder(
                        tween: Tween<double>(end: value),
                        duration: const Duration(milliseconds: 200),
                        curve: Curves.linear,
                        builder: (BuildContext context, double value, Widget? child) {
                          return Slider(
                            value: value,
                            onChanged: (v) {
                              if (dur <= 0) {
                                return;
                              }
                              widget.onInteract();
                              _dragValue.value = v;
                            },
                            onChangeEnd: dur > 0
                                ? (v) {
                                    controller.seek(Duration(milliseconds: (v * dur).round()));
                                    _dragValue.value = null;
                                  }
                                : null,
                          );
                        },
                      );
                    },
                  ),
                ),
                Row(
                  children: [
                    SignalBuilder(
                      builder: (context) => Text(
                        '${_formatDuration(controller.position.value)} / ${_formatDuration(controller.duration.value)}',
                        style: TextStyle(color: theme.textColor, fontSize: 12),
                      ),
                    ),
                    const Spacer(),
                    SignalBuilder(
                      builder: (context) => IconButton(
                        visualDensity: VisualDensity.compact,
                        iconSize: theme.secondaryIconSize,
                        color: theme.iconColor,
                        icon: Icon(
                          controller.muted.value || controller.volume.value == 0
                              ? Icons.volume_off
                              : Icons.volume_up,
                        ),
                        onPressed: () {
                          widget.onInteract();
                          controller.toggleMuted();
                        },
                      ),
                    ),
                    SignalBuilder(
                      builder: (context) => IconButton(
                        visualDensity: VisualDensity.compact,
                        iconSize: theme.secondaryIconSize,
                        color: controller.looping.value ? theme.activeIconColor : theme.iconColor,
                        icon: const Icon(Icons.loop),
                        onPressed: () {
                          widget.onInteract();
                          controller.setLooping(!controller.looping.value);
                        },
                      ),
                    ),
                    SignalBuilder(
                      builder: (context) => PopupMenuButton<double>(
                        tooltip: 'Playback speed',
                        initialValue: controller.speed.value,
                        onSelected: (v) {
                          widget.onInteract();
                          controller.setSpeed(v);
                        },
                        itemBuilder: (context) => [
                          for (final s in _speeds)
                            PopupMenuItem<double>(value: s, child: Text('${s}x')),
                        ],
                        child: Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 8),
                          child: Text(
                            '${controller.speed.value}x',
                            style: TextStyle(
                              color: theme.iconColor,
                              fontSize: theme.secondaryIconSize * 0.7,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class _CupertinoVideoControls extends StatefulWidget {
  const _CupertinoVideoControls({
    required this.controller,
    required this.theme,
    required this.onInteract,
  });

  final XueHuaPlayerController controller;
  final VideoControlsTheme theme;
  final VoidCallback onInteract;

  @override
  State<_CupertinoVideoControls> createState() => _CupertinoVideoControlsState();
}

class _CupertinoVideoControlsState extends State<_CupertinoVideoControls> {
  final FlutterSignal<double?> _dragValue = signal(null);

  Future<void> _showSpeedSheet() async {
    widget.onInteract();
    final c = widget.controller;
    await showCupertinoModalPopup<void>(
      context: context,
      builder: (context) => CupertinoActionSheet(
        title: const Text('Playback speed'),
        actions: [
          for (final s in _speeds)
            CupertinoActionSheetAction(
              onPressed: () {
                c.setSpeed(s);
                Navigator.of(context).pop();
              },
              child: Text('${s}x'),
            ),
        ],
        cancelButton: CupertinoActionSheetAction(
          isDefaultAction: true,
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final XueHuaPlayerController controller = widget.controller;
    final VideoControlsTheme theme = widget.theme;
    return Stack(
      children: [
        _CenterButton(
          controller: controller,
          theme: theme,
          onInteract: widget.onInteract,
          cupertino: true,
        ),
        Positioned(
          left: 8,
          right: 8,
          bottom: 8,
          child: LiquidGlassLens(
            style: LiquidGlassStyle(
              shape: LiquidGlassShape(
                cornerRadius: theme.borderRadius,
                lightColor: theme.backgroundColor,
              ),
            ),
            child: Container(
              // color: theme.backgroundColor,
              padding: theme.barPadding,
              child: Row(
                children: [
                  SignalBuilder(
                    builder: (context) => GestureDetector(
                      onTap: () {
                        widget.onInteract();
                        controller.toggleMuted();
                      },
                      child: Icon(
                        controller.muted.value || controller.volume.value == 0
                            ? CupertinoIcons.volume_off
                            : CupertinoIcons.volume_up,
                        size: theme.secondaryIconSize,
                        color: theme.iconColor,
                      ),
                    ),
                  ),
                  const SizedBox(width: 10),
                  SignalBuilder(
                    builder: (context) => Text(
                      _formatDuration(controller.position.value),
                      style: TextStyle(color: theme.textColor, fontSize: 12),
                    ),
                  ),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 8),
                      child: SignalBuilder(
                        builder: (context) {
                          final dur = controller.duration.value.inMilliseconds.toDouble();
                          final pos = controller.position.value.inMilliseconds.toDouble();
                          final value =
                              _dragValue.value ?? (dur > 0 ? (pos / dur).clamp(0.0, 1.0) : 0.0);
                          return TweenAnimationBuilder(
                            tween: Tween<double>(end: value),
                            duration: const Duration(milliseconds: 200),
                            curve: Curves.linear,
                            builder: (context, animatedValue, child) {
                              return CupertinoSlider(
                                value: animatedValue,
                                activeColor: theme.activeTrackColor,
                                thumbColor: theme.thumbColor,
                                onChanged: (value) {
                                  if (dur <= 0) {
                                    return;
                                  }
                                  widget.onInteract();
                                  _dragValue.value = value;
                                },
                                onChangeEnd: dur > 0
                                    ? (v) {
                                        controller.seek(Duration(milliseconds: (v * dur).round()));
                                        _dragValue.value = null;
                                      }
                                    : null,
                              );
                            },
                          );
                        },
                      ),
                    ),
                  ),
                  SignalBuilder(
                    builder: (context) => Text(
                      _formatDuration(controller.duration.value),
                      style: TextStyle(color: theme.textColor, fontSize: 12),
                    ),
                  ),
                  const SizedBox(width: 10),
                  SignalBuilder(
                    builder: (context) => GestureDetector(
                      onTap: () {
                        widget.onInteract();
                        controller.setLooping(!controller.looping.value);
                      },
                      child: Icon(
                        CupertinoIcons.repeat,
                        size: theme.secondaryIconSize,
                        color: controller.looping.value ? theme.activeIconColor : theme.iconColor,
                      ),
                    ),
                  ),
                  const SizedBox(width: 10),
                  GestureDetector(
                    onTap: _showSpeedSheet,
                    child: SignalBuilder(
                      builder: (context) => Text(
                        '${controller.speed.value}x',
                        style: TextStyle(
                          color: theme.iconColor,
                          fontSize: theme.secondaryIconSize * 0.7,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}
