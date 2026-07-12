import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:signals/signals_flutter.dart';

import '../enum/video_rotation.dart';
import '../controls/playback_controls_model.dart';
import '../presentation/playback_presentation_model.dart';
import '../media/frame_image.dart';
import '../media/media_source_resolver.dart';
import '../model/video_source.dart';
import '../domain/player_events.dart';
import '../surface/texture_surface.dart';
import 'command_port.dart';

/// 深度编排模块：signals、事件分发、open 生命周期与 transport / Deep orchestration: signals, event dispatch, open lifecycle, transport.
///
/// [XueHuaPlayerController] 的核心实现。维护 reactive 状态，订阅 [PlayerCommandPort.events]，
/// 将 [PlayerEvent] 映射到 signals；命令经 `_guard` 捕获异常并写入 [error]。
/// Core of [XueHuaPlayerController]. Maintains reactive state, listens to [PlayerCommandPort.events],
/// maps [PlayerEvent] to signals; commands run through `_guard` to capture errors into [error].
///
/// Seek/volume 等命令会先乐观更新 UI（`_preview*`），再异步调用 Rust。
/// Seek/volume and similar commands optimistically update UI (`_preview*`) before async Rust calls.
class PlaybackSession
    implements PlaybackControlsModel, PlaybackPresentationModel {
  /// 创建会话；可注入测试用 [port] 与 [mediaSourceResolver] / Creates a session with optional test doubles.
  PlaybackSession({
    PlayerCommandPort? port,
    MediaSourceResolver? mediaSourceResolver,
  }) : _port = port ?? ProductionPlayerCommandPort(),
       _mediaSourceResolver =
           mediaSourceResolver ?? const MediaSourceResolver();

  final PlayerCommandPort _port;

  final MediaSourceResolver _mediaSourceResolver;

  final FlutterSignal<PlayerState> _state = signal(PlayerState.idle);

  final FlutterSignal<Duration> _position = signal(Duration.zero);

  final FlutterSignal<Duration> _duration = signal(Duration.zero);

  final FlutterSignal<Size> _videoSize = signal(Size.zero);

  final FlutterSignal<int> _bufferingPercent = signal(100);

  final FlutterSignal<double> _volume = signal(1.0);

  final FlutterSignal<double> _speed = signal(1.0);

  final FlutterSignal<bool> _looping = signal(false);

  final FlutterSignal<bool> _muted = signal(false);

  final FlutterSignal<String?> _error = signal<String?>(null);

  final FlutterSignal<int?> _playerId = signal<int?>(null);

  final FlutterSignal<bool> _initialized = signal(false);

  final FlutterSignal<List<MediaTrack>> _tracks = signal(const []);

  final FlutterSignal<VideoMetadata?> _videoMetadata = signal(null);

  final FlutterSignal<bool> _isSeekable = signal(true);

  final FlutterSignal<bool> _supportsTracks = signal(true);

  final FlutterSignal<bool> _supportsOrientation = signal(false);

  final FlutterSignal<int> _mediaGeneration = signal(0);

  final FlutterSignal<VideoRotation> _videoRotation = signal(
    VideoRotation.deg0,
  );

  /// 每次 [open] 递增；供 View 在切换媒体时重置 UI 状态 / Increments on each [open]; lets views reset UI state on media switch.
  late final ReadonlySignal<int> mediaGeneration = _mediaGeneration;

  /// 是否正在播放 / Whether `state == playing`.
  @override
  late final ReadonlySignal<bool> isPlaying = computed(
    () => _state.value == PlayerState.playing,
  );

  /// 是否已 EOS / Whether playback completed.
  late final ReadonlySignal<bool> isCompleted = computed(
    () => _state.value == PlayerState.completed,
  );

  /// 显示宽高比；优先 DAR，否则由 [videoSize] 推算（均为 post-orient 尺寸）。
  /// Display aspect from DAR or [videoSize] (post-orient pipeline size).
  @override
  late final ReadonlySignal<double> aspectRatio = computed(() {
    final meta = _videoMetadata.value;
    if (meta != null &&
        meta.displayAspectWidth > 0 &&
        meta.displayAspectHeight > 0) {
      return meta.displayAspectWidth / meta.displayAspectHeight;
    }
    final s = _videoSize.value;
    return (s.width > 0 && s.height > 0) ? s.width / s.height : 16 / 9;
  });

  StreamSubscription<PlayerEvent>? _sub;
  bool _disposed = false;

  @override
  ReadonlySignal<bool> get initialized => _initialized;
  @override
  ReadonlySignal<int?> get playerId => _playerId;
  @override
  ReadonlySignal<PlayerState> get state => _state;
  @override
  ReadonlySignal<Duration> get position => _position;
  @override
  ReadonlySignal<Duration> get duration => _duration;
  ReadonlySignal<Size> get videoSize => _videoSize;
  @override
  ReadonlySignal<int> get bufferingPercent => _bufferingPercent;
  @override
  ReadonlySignal<double> get volume => _volume;
  @override
  ReadonlySignal<double> get speed => _speed;
  @override
  ReadonlySignal<bool> get looping => _looping;
  @override
  ReadonlySignal<bool> get muted => _muted;
  ReadonlySignal<String?> get error => _error;
  ReadonlySignal<List<MediaTrack>> get tracks => _tracks;
  ReadonlySignal<VideoMetadata?> get videoMetadata => _videoMetadata;
  @override
  ReadonlySignal<bool> get isSeekable => _isSeekable;
  ReadonlySignal<bool> get supportsTracks => _supportsTracks;
  @override
  ReadonlySignal<bool> get supportsOrientation => _supportsOrientation;
  @override
  ReadonlySignal<VideoRotation> get videoRotation => _videoRotation;

  /// 创建原生 player 并订阅事件流 / Creates native player and subscribes to events.
  Future<void> initialize() async {
    if (_initialized.value) return;
    await _port.create();
    final id = _port.playerId;
    if (id == null) {
      throw StateError('PlayerCommandPort.create() did not assign playerId');
    }
    _playerId.value = id;
    _initialized.value = true;
    _sub = _port.events.listen(
      _onEvent,
      onError: (Object e) => _applyError(e.toString()),
    );
  }

  void _onEvent(PlayerEvent event) {
    if (_disposed) return;
    if (event.kind == PlayerEventKind.tracksChanged) {
      unawaited(_refreshTracksFromPort());
      return;
    }
    _applyEvent(event);
  }

  Future<void> _guard(Future<void> Function() action) async {
    try {
      await action();
    } catch (e) {
      _applyError(e.toString());
    }
  }

  /// 经统一解析器加载 [source] / Loads [source] via the unified media resolver.
  ///
  /// 调用前 [_resetForOpen] 清空上一媒体状态，并 [setVideoRotation] 同步 native 为 0°；
  /// 加载后更新 pipeline 能力并刷新轨道。
  /// Clears prior media state via [_resetForOpen]; resets native rotation before load.
  Future<void> open(VideoSource source, {bool autoPlay = false}) async {
    _resetForOpen();
    await _guard(() async {
      await _port.setVideoRotation(0);
      await _port.loadSource(
        _mediaSourceResolver.resolve(source),
        autoPlay: autoPlay,
      );
      _setPipelineCapabilities(await _port.getPipelineCapabilities());
      await _refreshTracksFromPort();
    });
  }

  /// 播放；EOS 后手动 replay 会将 [speed] 重置为 1.0 并从 0 起播 / Plays; resets speed and position after EOS replay.
  Future<void> play() {
    // Manual replay after EOS resets speed to 1x (engine resets its rate too);
    // keep the UI in sync. Normal pause->resume (not completed) keeps the speed.
    if (_state.value == PlayerState.completed) {
      _speed.value = 1.0;
      _position.value = Duration.zero;
    }
    return _guard(_port.play);
  }

  Future<void> pause() => _guard(_port.pause);

  Future<void> stop() => _guard(_port.stop);

  @override
  Future<void> togglePlayPause() => isPlaying.value ? pause() : play();

  /// 跳转；仅更新位置预览，缓冲态由 native BUFFERING 事件驱动 / Seeks; position preview only — buffering from native events.
  @override
  Future<void> seek(Duration position) async {
    _previewSeek(position, showBuffering: false);
    await _guard(() => _port.seek(position));
  }

  @override
  Future<void> setVolume(double volume) async {
    _previewVolume(volume);
    await _guard(() => _port.setVolume(_volume.value));
  }

  Future<void> setMuted(bool muted) async {
    _previewMuted(muted);
    await _guard(() => _port.setMute(muted));
  }

  @override
  Future<void> toggleMuted() => setMuted(!_muted.value);

  @override
  Future<void> setSpeed(double speed) async {
    _previewSpeed(speed);
    await _guard(() => _port.setSpeed(_speed.value));
  }

  @override
  Future<void> setLooping(bool looping) async {
    _previewLooping(looping);
    await _guard(() => _port.setLooping(looping));
  }

  /// 从 port 重新拉取轨道 / Refreshes tracks from the port.
  Future<void> refreshTracks() => _refreshTracksFromPort();

  Future<void> selectTrack(MediaTrack track, {bool enable = true}) =>
      _guard(() => _port.selectTrack(track, enable: enable));

  @override
  Future<void> setVideoRotation(VideoRotation rotation) async {
    _videoRotation.value = rotation;
    await _guard(() => _port.setVideoRotation(rotation.degrees));
  }

  @override
  Future<void> setAspectRatioMode(AspectRatioMode mode) =>
      _guard(() => _port.setAspectRatioMode(mode));

  /// 截取当前最新画面为 PNG / Captures the latest decoded frame as PNG.
  Future<Uint8List> captureCurrentFrame() async {
    final map = await _port.captureCurrentFrame();
    return CapturedBgraFrame.fromMap(map).toPng();
  }

  /// 取消订阅、销毁 player 并释放全部 signals / Cancels subscription, disposes player and all signals.
  ///
  /// Releases the native Flutter texture while [playerId] is still valid, then
  /// nulls signals before awaiting port dispose so late events cannot race.
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    final id = _playerId.value;
    if (id != null) {
      await disposeNativePlayerTexture(id);
    }
    _playerId.value = null;
    _initialized.value = false;
    await _sub?.cancel();
    await _port.dispose();
    isPlaying.dispose();
    isCompleted.dispose();
    aspectRatio.dispose();
    _state.dispose();
    _position.dispose();
    _duration.dispose();
    _videoSize.dispose();
    _bufferingPercent.dispose();
    _volume.dispose();
    _speed.dispose();
    _looping.dispose();
    _muted.dispose();
    _error.dispose();
    _playerId.dispose();
    _initialized.dispose();
    _tracks.dispose();
    _videoMetadata.dispose();
    _isSeekable.dispose();
    _supportsTracks.dispose();
    _supportsOrientation.dispose();
    _videoRotation.dispose();
    _mediaGeneration.dispose();
  }

  Future<void> _refreshTracksFromPort() async {
    try {
      _tracks.value = await _port.getTracks();
    } catch (e) {
      _applyError(e.toString());
    }
  }

  void _resetForOpen() {
    batch(() {
      _error.value = null;
      // Optimistic loading UI until native BUFFERING / READY / PLAYING events.
      _bufferingPercent.value = 0;
      _videoSize.value = Size.zero;
      _videoMetadata.value = null;
      _tracks.value = const [];
      _speed.value = 1.0;
      _state.value = PlayerState.buffering;
      _position.value = Duration.zero;
      _duration.value = Duration.zero;
      _isSeekable.value = true;
      _volume.value = 1.0;
      _muted.value = false;
      _looping.value = false;
      _videoRotation.value = VideoRotation.deg0;
      _mediaGeneration.value++;
    });
  }

  void _applyEvent(PlayerEvent event) {
    switch (event.kind) {
      case PlayerEventKind.durationChanged:
        _duration.value = Duration(milliseconds: event.durationMs);
      case PlayerEventKind.positionChanged:
        _position.value = Duration(milliseconds: event.positionMs);
      case PlayerEventKind.videoSize:
        _videoSize.value = Size(
          event.width.toDouble(),
          event.height.toDouble(),
        );
      case PlayerEventKind.metadataChanged:
        _videoMetadata.value = VideoMetadata(
          width: event.width,
          height: event.height,
          fps: event.fps,
          pixelAspectWidth: event.pixelAspectWidth,
          pixelAspectHeight: event.pixelAspectHeight,
          displayAspectWidth: event.displayAspectWidth,
          displayAspectHeight: event.displayAspectHeight,
          interlaced: event.interlaced,
          colorMatrix: event.colorMatrix,
          colorRange: event.colorRange,
          hdrFormat: event.hdrFormat,
        );
      case PlayerEventKind.stateChanged:
        _state.value = event.state;
      case PlayerEventKind.buffering:
        _bufferingPercent.value = event.bufferingPercent;
        // Native may pause during rebuffer; restore transport state from the event.
        if (event.bufferingPercent < 100) {
          _state.value = PlayerState.buffering;
        } else {
          // Buffering finished but native may still be waiting for surface /
          // deferred play — do not fake playing (masks "not actually playing").
          _state.value = event.state == PlayerState.buffering
              ? PlayerState.ready
              : event.state;
        }
      case PlayerEventKind.eos:
        batch(() {
          _state.value = PlayerState.completed;
          _position.value = _duration.value;
        });
      case PlayerEventKind.error:
        batch(() {
          _error.value = event.message;
          _state.value = PlayerState.error;
          _bufferingPercent.value = 100;
        });
      case PlayerEventKind.tracksChanged:
        break;
    }
  }

  void _applyError(String message) {
    batch(() {
      _error.value = message;
      _state.value = PlayerState.error;
      _bufferingPercent.value = 100;
    });
  }

  void _setPipelineCapabilities(PipelineCapabilitiesDto caps) {
    _isSeekable.value = caps.seek;
    _supportsTracks.value = caps.tracks;
    _supportsOrientation.value = caps.orientation;
  }

  void _previewSeek(Duration position, {required bool showBuffering}) {
    _position.value = position;
    if (showBuffering) {
      _state.value = PlayerState.buffering;
    }
  }

  void _previewVolume(double volume) {
    final v = volume.clamp(0.0, 1.0);
    _volume.value = v;
    if (v > 0 && _muted.value) {
      _muted.value = false;
    }
  }

  void _previewMuted(bool muted) {
    _muted.value = muted;
  }

  void _previewSpeed(double speed) {
    _speed.value = speed <= 0 ? 1.0 : speed;
  }

  void _previewLooping(bool looping) {
    _looping.value = looping;
  }
}
