import 'package:signals/signals_flutter.dart';
import 'package:xue_hua_video_player/src/presentation/playback_presentation_model.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';

/// Test double for [PlaybackPresentationModel].
class FakePlaybackPresentationModel implements PlaybackPresentationModel {
  FakePlaybackPresentationModel({
    int? playerId = 42,
    bool initialized = true,
    double aspectRatio = 16 / 9,
    PlayerState state = PlayerState.idle,
    int bufferingPercent = 100,
  }) : _playerId = signal(playerId),
       _initialized = signal(initialized),
       _aspectRatio = signal(aspectRatio),
       _state = signal(state),
       _bufferingPercent = signal(bufferingPercent);

  final FlutterSignal<int?> _playerId;
  final FlutterSignal<bool> _initialized;
  final FlutterSignal<double> _aspectRatio;
  final FlutterSignal<PlayerState> _state;
  final FlutterSignal<int> _bufferingPercent;

  AspectRatioMode? lastAspectRatioMode;
  int setAspectRatioModeCallCount = 0;

  @override
  ReadonlySignal<bool> get initialized => _initialized;

  @override
  ReadonlySignal<int?> get playerId => _playerId;

  @override
  ReadonlySignal<double> get aspectRatio => _aspectRatio;

  @override
  ReadonlySignal<PlayerState> get state => _state;

  @override
  ReadonlySignal<int> get bufferingPercent => _bufferingPercent;

  @override
  Future<void> setAspectRatioMode(AspectRatioMode mode) async {
    setAspectRatioModeCallCount++;
    lastAspectRatioMode = mode;
  }

  void setState(PlayerState value) => _state.value = value;

  void setBufferingPercent(int value) => _bufferingPercent.value = value;

  void setAspectRatio(double value) => _aspectRatio.value = value;

  void setPlayerId(int? value) => _playerId.value = value;

  void dispose() {
    _playerId.dispose();
    _initialized.dispose();
    _aspectRatio.dispose();
    _state.dispose();
    _bufferingPercent.dispose();
  }
}
