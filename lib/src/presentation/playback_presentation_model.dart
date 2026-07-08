import 'package:signals/signals_flutter.dart';

import '../rust/player_events.dart';

/// Narrow seam for video presentation: surface routing, layout aspect, loading chrome.
abstract class PlaybackPresentationModel {
  ReadonlySignal<bool> get initialized;
  ReadonlySignal<int?> get playerId;
  ReadonlySignal<double> get aspectRatio;
  ReadonlySignal<PlayerState> get state;
  ReadonlySignal<int> get bufferingPercent;

  Future<void> setAspectRatioMode(AspectRatioMode mode);
}
