import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../xue_hua_player_controller.dart';

/// Shared drag/seek handling for the built-in sliders.
///
/// While the user drags, the slider is pinned to the finger position. On
/// release the value stays pinned to the requested target and only unpins once
/// the reported [XueHuaPlayerController.position] catches up (or after a safety
/// timeout), so the thumb never bounces back to the stale play position while
/// the async seek is in flight.
mixin SeekMixin<T extends StatefulWidget> on State<T> {
  final FlutterSignal<double?> _dragValue = signal(null);
  bool _seeking = false;
  Timer? _seekTimeout;
  void Function()? _seekWatch;

  XueHuaPlayerController get seekController;

  VoidCallback get onSeekInteract;

  @override
  void initState() {
    super.initState();
    _seekWatch = effect(_checkSeekSettled);
  }

  @override
  void dispose() {
    _seekTimeout?.cancel();
    _seekWatch?.call();
    super.dispose();
  }

  void _checkSeekSettled() {
    final target = _dragValue.value;
    final dur = seekController.duration.value.inMilliseconds.toDouble();
    final pos = seekController.position.value.inMilliseconds.toDouble();
    if (target == null || !_seeking || dur <= 0) return;
    final toleranceMs = math.max(400.0, dur * 0.01);
    if ((pos - target * dur).abs() <= toleranceMs) {
      _clearSeek();
    }
  }

  void _clearSeek() {
    _seekTimeout?.cancel();
    _seekTimeout = null;
    _seeking = false;
    _dragValue.value = null;
  }

  /// The fraction the slider should currently display.
  double sliderValue(double durMs, double posMs) {
    final target = _dragValue.value;
    if (target != null) return target;
    return durMs > 0 ? (posMs / durMs).clamp(0.0, 1.0) : 0.0;
  }

  /// Whether the user is dragging or an async seek is still settling.
  bool get isScrubbing => _dragValue.value != null;

  void onSeekChanged(double v, double durMs) {
    if (durMs <= 0) return;
    onSeekInteract();
    _dragValue.value = v;
  }

  void onSeekEnd(double v, double durMs) {
    if (durMs <= 0) return;
    _dragValue.value = v;
    _seeking = true;
    seekController.seek(Duration(milliseconds: (v * durMs).round()));
    _seekTimeout?.cancel();
    _seekTimeout = Timer(const Duration(milliseconds: 1500), _clearSeek);
  }
}
