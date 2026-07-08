import 'package:signals/signals_flutter.dart';

import '../rust/player_events.dart';

/// Narrow seam for built-in video controls: readonly transport state + commands.
abstract class PlaybackControlsModel {
  ReadonlySignal<PlayerState> get state;
  ReadonlySignal<int> get bufferingPercent;
  ReadonlySignal<bool> get isPlaying;
  ReadonlySignal<Duration> get position;
  ReadonlySignal<Duration> get duration;
  ReadonlySignal<bool> get isSeekable;
  ReadonlySignal<bool> get muted;
  ReadonlySignal<double> get volume;
  ReadonlySignal<bool> get looping;
  ReadonlySignal<double> get speed;

  Future<void> togglePlayPause();
  Future<void> toggleMuted();
  Future<void> setLooping(bool looping);
  Future<void> setSpeed(double speed);
  Future<void> seek(Duration position);
}
