import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:liquid_glass_widgets/liquid_glass_widgets.dart';
import 'package:signals/signals_flutter.dart';

import '../constant/constant.dart';
import '../theme/video_controls_theme.dart';
import '../utils/time_util.dart';
import 'center_button.dart';
import 'playback_controls_model.dart';
import 'playback_progress_slider.dart';
import 'scrub_controller.dart';

class CupertinoVideoControls extends StatefulWidget {
  const CupertinoVideoControls({
    super.key,
    required this.model,
    required this.theme,
    required this.onInteract,
  });

  final PlaybackControlsModel model;
  final VideoControlsTheme theme;
  final VoidCallback onInteract;

  @override
  State<CupertinoVideoControls> createState() => _CupertinoVideoControlsState();
}

class _CupertinoVideoControlsState extends State<CupertinoVideoControls> {
  late final ScrubController _scrub;

  @override
  void initState() {
    super.initState();
    _scrub = ScrubController(
      model: widget.model,
      onInteract: widget.onInteract,
    );
  }

  @override
  void dispose() {
    _scrub.dispose();
    super.dispose();
  }

  Future<void> _showSpeedSheet() async {
    widget.onInteract();
    final model = widget.model;
    await showCupertinoModalPopup<void>(
      context: context,
      builder: (context) => CupertinoActionSheet(
        actions: [
          for (final s in speeds)
            CupertinoActionSheetAction(
              onPressed: () {
                model.setSpeed(s);
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
    final model = widget.model;
    final theme = widget.theme;
    final size = MediaQuery.sizeOf(context);
    return Stack(
      children: [
        CenterButton(model: model, theme: theme, onInteract: widget.onInteract),
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
                        model.toggleMuted();
                      },
                      style: IconButton.styleFrom(
                        tapTargetSize: .shrinkWrap,
                        visualDensity: .compact,
                      ),
                      icon: Icon(
                        model.muted.value || model.volume.value == 0
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
                      formatDuration(model.position.value),
                      style: TextStyle(color: theme.textColor, fontSize: 12),
                    ),
                  ),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 8),
                      child: PlaybackProgressSlider(
                        model: model,
                        scrub: _scrub,
                        builder: (context, snap) => GlassSlider(
                          value: snap.displayValue,
                          activeColor: theme.activeTrackColor,
                          thumbColor: theme.thumbColor,
                          onChangeStart: snap.enabled
                              ? (_) => snap.onSeekStart?.call()
                              : null,
                          onChanged: snap.onSeekChanged,
                          onChangeEnd: snap.onSeekEnd,
                        ),
                      ),
                    ),
                  ),
                  SignalBuilder(
                    builder: (context) => Text(
                      formatDuration(model.duration.value),
                      style: TextStyle(color: theme.textColor, fontSize: 12),
                    ),
                  ),
                  const SizedBox(width: 10),
                  SignalBuilder(
                    builder: (context) => IconButton(
                      onPressed: () async {
                        widget.onInteract();
                        await model.setLooping(!model.looping.value);
                      },
                      style: IconButton.styleFrom(
                        tapTargetSize: .shrinkWrap,
                        visualDensity: .compact,
                      ),
                      icon: Icon(
                        CupertinoIcons.repeat,
                        size: theme.secondaryIconSize,
                        color: model.looping.value
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
                        '${model.speed.value}x',
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
