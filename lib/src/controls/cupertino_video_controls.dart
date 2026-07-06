import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:liquid_glass_widgets/liquid_glass_widgets.dart';
import 'package:signals/signals_flutter.dart';

import '../constant/constant.dart';
import '../mixin/seek_mixin.dart';
import '../xue_hua_player_controller.dart';
import '../theme/video_controls_theme.dart';
import '../utils/time_util.dart';
import 'center_button.dart';

class CupertinoVideoControls extends StatefulWidget {
  const CupertinoVideoControls({
    super.key,
    required this.controller,
    required this.theme,
    required this.onInteract,
  });

  final XueHuaPlayerController controller;
  final VideoControlsTheme theme;
  final VoidCallback onInteract;

  @override
  State<CupertinoVideoControls> createState() => _CupertinoVideoControlsState();
}

class _CupertinoVideoControlsState extends State<CupertinoVideoControls>
    with SeekMixin {
  @override
  XueHuaPlayerController get seekController => widget.controller;

  @override
  VoidCallback get onSeekInteract => widget.onInteract;

  Future<void> _showSpeedSheet() async {
    widget.onInteract();
    final c = widget.controller;
    await showCupertinoModalPopup<void>(
      context: context,
      builder: (context) => CupertinoActionSheet(
        actions: [
          for (final s in speeds)
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
    final Size size = MediaQuery.sizeOf(context);
    return Stack(
      children: [
        CenterButton(
          controller: controller,
          theme: theme,
          onInteract: widget.onInteract,
        ),
        Positioned(
          left: 8,
          right: 8,
          bottom: 8,
          child: SafeArea(
            top: false,
            child: GlassCard(
              width: size.width - 16,
              padding: theme.barPadding,
              child: Row(
                children: [
                  SignalBuilder(
                    builder: (context) => IconButton(
                      onPressed: () {
                        widget.onInteract();
                        controller.toggleMuted();
                      },
                      style: IconButton.styleFrom(
                        tapTargetSize: .shrinkWrap,
                        visualDensity: .compact,
                      ),
                      icon: Icon(
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
                      formatDuration(controller.position.value),
                      style: TextStyle(color: theme.textColor, fontSize: 12),
                    ),
                  ),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 8),
                      child: SignalBuilder(
                        builder: (context) {
                          final dur = controller.duration.value.inMilliseconds
                              .toDouble();
                          final pos = controller.position.value.inMilliseconds
                              .toDouble();
                          final seekable = controller.isSeekable.value;
                          final value = sliderValue(dur, pos);
                          Widget buildSlider(double v) {
                            return GlassSlider(
                              value: v,
                              activeColor: theme.activeTrackColor,
                              thumbColor: theme.thumbColor,
                              onChangeStart: seekable && dur > 0
                                  ? (_) => onSeekStart()
                                  : null,
                              onChanged: seekable && dur > 0
                                  ? (v) => onSeekChanged(v, dur)
                                  : null,
                              onChangeEnd: seekable && dur > 0
                                  ? (v) => onSeekEnd(v, dur)
                                  : null,
                            );
                          }

                          return TweenAnimationBuilder<double>(
                            tween: Tween<double>(end: value),
                            duration: isScrubbing
                                ? Duration.zero
                                : const Duration(milliseconds: 200),
                            curve: Curves.linear,
                            builder: (context, animatedValue, _) =>
                                buildSlider(
                              isScrubbing ? value : animatedValue,
                            ),
                          );
                        },
                      ),
                    ),
                  ),
                  SignalBuilder(
                    builder: (context) => Text(
                      formatDuration(controller.duration.value),
                      style: TextStyle(color: theme.textColor, fontSize: 12),
                    ),
                  ),
                  const SizedBox(width: 10),
                  SignalBuilder(
                    builder: (context) => IconButton(
                      onPressed: () async {
                        widget.onInteract();
                        await controller.setLooping(!controller.looping.value);
                      },
                      style: IconButton.styleFrom(
                        tapTargetSize: .shrinkWrap,
                        visualDensity: .compact,
                      ),
                      icon: Icon(
                        CupertinoIcons.repeat,
                        size: theme.secondaryIconSize,
                        color: controller.looping.value
                            ? theme.iconColor
                            : theme.iconColor.withValues(alpha: 0.5),
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
