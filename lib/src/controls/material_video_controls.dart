import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../constant/constant.dart';
import '../mixin/seek_mixin.dart';
import '../xue_hua_player_controller.dart';
import '../theme/video_controls_theme.dart';
import '../utils/time_util.dart';
import 'center_button.dart';

class MaterialVideoControls extends StatefulWidget {
  const MaterialVideoControls({
    super.key,
    required this.controller,
    required this.theme,
    required this.onInteract,
  });

  final XueHuaPlayerController controller;
  final VideoControlsTheme theme;
  final VoidCallback onInteract;

  @override
  State<MaterialVideoControls> createState() => _MaterialVideoControlsState();
}

class _MaterialVideoControlsState extends State<MaterialVideoControls>
    with SeekMixin {
  @override
  XueHuaPlayerController get seekController => widget.controller;

  @override
  VoidCallback get onSeekInteract => widget.onInteract;

  @override
  Widget build(BuildContext context) {
    final XueHuaPlayerController controller = widget.controller;
    final VideoControlsTheme theme = widget.theme;
    return Stack(
      children: [
        CenterButton(
          controller: controller,
          theme: theme,
          onInteract: widget.onInteract,
        ),
        Positioned(
          left: 0,
          right: 0,
          bottom: 0,
          child: SafeArea(
            top: false,
            child: Container(
              padding: theme.barPadding,
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment.bottomCenter,
                  end: Alignment.topCenter,
                  colors: [
                    theme.backgroundColor,
                    theme.backgroundColor.withValues(alpha: 0),
                  ],
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
                      overlayShape: const RoundSliderOverlayShape(
                        overlayRadius: 12,
                      ),
                      thumbShape: const RoundSliderThumbShape(
                        enabledThumbRadius: 6,
                      ),
                    ),
                    child: SignalBuilder(
                      builder: (context) {
                        final dur = controller.duration.value.inMilliseconds
                            .toDouble();
                        final pos = controller.position.value.inMilliseconds
                            .toDouble();
                        final seekable = controller.isSeekable.value;
                        final value = sliderValue(dur, pos);
                        Widget buildSlider(double v) {
                          return Slider(
                            value: v,
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
                              buildSlider(isScrubbing ? value : animatedValue),
                        );
                      },
                    ),
                  ),
                  Row(
                    children: [
                      SignalBuilder(
                        builder: (context) => Text(
                          '${formatDuration(controller.position.value)} / ${formatDuration(controller.duration.value)}',
                          style: TextStyle(
                            color: theme.textColor,
                            fontSize: 12,
                          ),
                        ),
                      ),
                      const Spacer(),
                      SignalBuilder(
                        builder: (context) => IconButton(
                          style: IconButton.styleFrom(
                            tapTargetSize: .shrinkWrap,
                            visualDensity: .compact,
                          ),
                          color: theme.iconColor,
                          icon: Icon(
                            controller.muted.value ||
                                    controller.volume.value == 0
                                ? Icons.volume_off
                                : Icons.volume_up,
                            size: theme.secondaryIconSize,
                          ),
                          onPressed: () {
                            widget.onInteract();
                            controller.toggleMuted();
                          },
                        ),
                      ),
                      SignalBuilder(
                        builder: (context) => IconButton(
                          style: IconButton.styleFrom(
                            tapTargetSize: .shrinkWrap,
                            visualDensity: .compact,
                          ),
                          color: controller.looping.value
                              ? theme.iconColor
                              : theme.iconColor.withValues(alpha: 0.5),
                          icon: Icon(Icons.loop, size: theme.secondaryIconSize),
                          onPressed: () async {
                            widget.onInteract();
                            await controller.setLooping(
                              !controller.looping.value,
                            );
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
                            for (final s in speeds)
                              PopupMenuItem<double>(
                                value: s,
                                child: Text('${s}x'),
                              ),
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
        ),
      ],
    );
  }
}
