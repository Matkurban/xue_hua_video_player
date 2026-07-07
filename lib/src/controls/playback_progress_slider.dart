import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import 'playback_controls_model.dart';
import 'scrub_controller.dart';

/// Snapshot passed to [PlaybackProgressSlider] skin builders.
class PlaybackSliderSnapshot {
  const PlaybackSliderSnapshot({
    required this.displayValue,
    required this.enabled,
    required this.onSeekStart,
    required this.onSeekChanged,
    required this.onSeekEnd,
  });

  final double displayValue;
  final bool enabled;
  final VoidCallback? onSeekStart;
  final ValueChanged<double>? onSeekChanged;
  final ValueChanged<double>? onSeekEnd;
}

typedef PlaybackProgressSliderBuilder =
    Widget Function(BuildContext context, PlaybackSliderSnapshot snapshot);

/// Shared progress slider wiring: signals, scrub pinning, and settle animation.
class PlaybackProgressSlider extends StatelessWidget {
  const PlaybackProgressSlider({
    super.key,
    required this.model,
    required this.scrub,
    required this.builder,
  });

  final PlaybackControlsModel model;
  final ScrubController scrub;
  final PlaybackProgressSliderBuilder builder;

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        final dur = model.duration.value.inMilliseconds.toDouble();
        final pos = model.position.value.inMilliseconds.toDouble();
        final seekable = model.isSeekable.value;
        final enabled = seekable && dur > 0;
        final value = scrub.sliderValue(dur, pos);

        PlaybackSliderSnapshot snapshotFor(double v) {
          return PlaybackSliderSnapshot(
            displayValue: v,
            enabled: enabled,
            onSeekStart: enabled ? scrub.onSeekStart : null,
            onSeekChanged: enabled
                ? (fraction) => scrub.onSeekChanged(fraction, dur)
                : null,
            onSeekEnd: enabled
                ? (fraction) => scrub.onSeekEnd(fraction, dur)
                : null,
          );
        }

        return TweenAnimationBuilder<double>(
          tween: Tween<double>(end: value),
          duration: scrub.isScrubbing
              ? Duration.zero
              : const Duration(milliseconds: 200),
          curve: Curves.linear,
          builder: (context, animatedValue, _) => builder(
            context,
            snapshotFor(scrub.isScrubbing ? value : animatedValue),
          ),
        );
      },
    );
  }
}
