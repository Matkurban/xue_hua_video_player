import 'dart:async';
import 'dart:math' as math;
import 'dart:ui';

import 'package:signals/signals_flutter.dart';

import 'playback_controls_model.dart';

/// Drag/seek handling for built-in progress sliders.
///
/// While the user drags, the slider pins to the finger position. On release the
/// value stays pinned until [PlaybackControlsModel.position] catches up (or
/// after a safety timeout), so the thumb never bounces back to a stale position
/// while the async seek is in flight.
class ScrubController {
  ScrubController({required this.model, required this.onInteract}) {
    _seekWatch = effect(_checkSeekSettled);
  }

  static const dragSafetyMs = 300;
  static const seekSettleMs = 1500;

  final PlaybackControlsModel model;
  final VoidCallback onInteract;

  final FlutterSignal<double?> _dragValue = signal(null);
  bool _dragging = false;
  bool _seeking = false;
  Timer? _seekTimeout;
  late final void Function() _seekWatch;

  double _seekToleranceMs(double durMs) => math.max(400.0, durMs * 0.01);

  bool _isNearPosition(double fraction, double durMs, double posMs) {
    if (durMs <= 0) return true;
    return (posMs - fraction * durMs).abs() <= _seekToleranceMs(durMs);
  }

  void _checkSeekSettled() {
    final target = _dragValue.value;
    final dur = model.duration.value.inMilliseconds.toDouble();
    final pos = model.position.value.inMilliseconds.toDouble();
    if (target == null || dur <= 0) return;
    if (!_isNearPosition(target, dur, pos)) return;
    if (_seeking || !_dragging) {
      _clearSeek();
    }
  }

  void _clearSeek() {
    _seekTimeout?.cancel();
    _seekTimeout = null;
    _dragging = false;
    _seeking = false;
    _dragValue.value = null;
  }

  void _armDragSafetyTimeout() {
    _seekTimeout?.cancel();
    _seekTimeout = Timer(const Duration(milliseconds: dragSafetyMs), () {
      if (_dragging && !_seeking) _clearSeek();
    });
  }

  void _armSeekSettleTimeout() {
    _seekTimeout?.cancel();
    _seekTimeout = Timer(
      const Duration(milliseconds: seekSettleMs),
      _clearSeek,
    );
  }

  /// The fraction the slider should currently display.
  double sliderValue(double durMs, double posMs) {
    final target = _dragValue.value;
    if (target != null) return target;
    return durMs > 0 ? (posMs / durMs).clamp(0.0, 1.0) : 0.0;
  }

  /// Whether the user is dragging or an async seek is still settling.
  bool get isScrubbing => _dragValue.value != null;

  void onSeekStart() {
    onInteract();
    _dragging = true;
    _armDragSafetyTimeout();
  }

  void onSeekChanged(double v, double durMs) {
    if (durMs <= 0) return;
    _dragging = true;
    onInteract();
    _dragValue.value = v;
    _armDragSafetyTimeout();
  }

  void onSeekEnd(double v, double durMs) {
    if (durMs <= 0) return;
    _dragging = false;
    _dragValue.value = v;
    final pos = model.position.value.inMilliseconds.toDouble();
    if (_isNearPosition(v, durMs, pos)) {
      _clearSeek();
      return;
    }
    _seeking = true;
    model.seek(Duration(milliseconds: (v * durMs).round()));
    _armSeekSettleTimeout();
  }

  void dispose() {
    _seekTimeout?.cancel();
    _seekWatch();
    _dragValue.dispose();
  }
}
