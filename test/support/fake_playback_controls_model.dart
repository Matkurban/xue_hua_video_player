import 'package:signals/signals_flutter.dart';
import 'package:xue_hua_video_player/src/controls/playback_controls_model.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';

/// Test double for [PlaybackControlsModel].
class FakePlaybackControlsModel implements PlaybackControlsModel {
  FakePlaybackControlsModel({
    PlayerState initialState = PlayerState.idle,
    Duration initialPosition = Duration.zero,
    Duration initialDuration = const Duration(seconds: 100),
    bool initialSeekable = true,
    bool supportsOrientation = true,
    VideoOrientationConfig initialOrientation = const VideoOrientationConfig(
      flipHorizontal: false,
      flipVertical: false,
      rotateDegrees: 0,
    ),
  }) : _state = signal(initialState),
       _position = signal(initialPosition),
       _duration = signal(initialDuration),
       _isSeekable = signal(initialSeekable),
       _supportsOrientation = signal(supportsOrientation),
       _videoOrientation = signal(initialOrientation);

  final FlutterSignal<PlayerState> _state;
  final FlutterSignal<Duration> _position;
  final FlutterSignal<Duration> _duration;
  final FlutterSignal<bool> _isSeekable;
  final FlutterSignal<bool> _supportsOrientation;
  final FlutterSignal<VideoOrientationConfig> _videoOrientation;
  final FlutterSignal<bool> _muted = signal(false);
  final FlutterSignal<double> _volume = signal(1.0);
  final FlutterSignal<bool> _looping = signal(false);
  final FlutterSignal<double> _speed = signal(1.0);
  final FlutterSignal<int> _bufferingPercent = signal(100);

  @override
  late final ReadonlySignal<bool> isPlaying = computed(
    () => _state.value == PlayerState.playing,
  );

  Duration? lastSeek;
  int seekCallCount = 0;
  int togglePlayPauseCallCount = 0;
  VideoOrientationConfig? lastVideoOrientation;

  @override
  ReadonlySignal<PlayerState> get state => _state;

  @override
  ReadonlySignal<int> get bufferingPercent => _bufferingPercent;

  @override
  ReadonlySignal<Duration> get position => _position;

  @override
  ReadonlySignal<Duration> get duration => _duration;

  @override
  ReadonlySignal<bool> get isSeekable => _isSeekable;

  @override
  ReadonlySignal<bool> get muted => _muted;

  @override
  ReadonlySignal<double> get volume => _volume;

  @override
  ReadonlySignal<bool> get looping => _looping;

  @override
  ReadonlySignal<double> get speed => _speed;

  @override
  ReadonlySignal<bool> get supportsOrientation => _supportsOrientation;

  @override
  ReadonlySignal<VideoOrientationConfig> get videoOrientation =>
      _videoOrientation;

  @override
  Future<void> togglePlayPause() async {
    togglePlayPauseCallCount++;
    _state.value = _state.value == PlayerState.playing
        ? PlayerState.paused
        : PlayerState.playing;
  }

  @override
  Future<void> toggleMuted() async {}

  @override
  Future<void> setLooping(bool looping) async {
    _looping.value = looping;
  }

  @override
  Future<void> setSpeed(double speed) async {
    _speed.value = speed;
  }

  @override
  Future<void> seek(Duration position) async {
    seekCallCount++;
    lastSeek = position;
  }

  double? lastVolume;
  AspectRatioMode? lastAspectRatioMode;

  @override
  Future<void> setVolume(double volume) async {
    lastVolume = volume;
    _volume.value = volume;
  }

  @override
  Future<void> setAspectRatioMode(AspectRatioMode mode) async {
    lastAspectRatioMode = mode;
  }

  @override
  Future<void> setVideoOrientation(VideoOrientationConfig config) async {
    lastVideoOrientation = config;
    _videoOrientation.value = config;
  }

  void setPosition(Duration position) {
    _position.value = position;
  }

  void dispose() {
    isPlaying.dispose();
    _state.dispose();
    _position.dispose();
    _duration.dispose();
    _isSeekable.dispose();
    _supportsOrientation.dispose();
    _videoOrientation.dispose();
    _muted.dispose();
    _volume.dispose();
    _looping.dispose();
    _speed.dispose();
    _bufferingPercent.dispose();
  }
}
