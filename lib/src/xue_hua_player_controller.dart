import 'package:flutter/widgets.dart';
import 'package:signals/signals_flutter.dart';

import 'enum/video_rotation.dart';
import 'controls/immersive_controls_state.dart';
import 'controls/playback_controls_model.dart';
import 'presentation/playback_presentation_model.dart';
import 'media/media_source_resolver.dart';
import 'model/video_source.dart';
import 'player/command_port.dart';
import 'player/playback_session.dart';
import 'rust/player_events.dart';
import 'utils/platform_util.dart';

export 'enum/video_rotation.dart';
export 'rust/player_events.dart'
    show
        AspectRatioMode,
        MediaTrack,
        PipelineCapabilitiesDto,
        PlayerState,
        PlayerEvent,
        PlayerEventKind,
        TrackType,
        VideoMetadata;
export 'model/video_source.dart';

/// 单个 GStreamer 播放器的公开门面 / Public facade for a single GStreamer-backed player.
///
/// 编排委托给 [PlaybackSession]；实现 [PlaybackControlsModel] 与 [PlaybackPresentationModel]，
/// 供 [XueHuaVideoView] 与 [VideoControls] 直接绑定。
/// Delegates orchestration to [PlaybackSession]; implements [PlaybackControlsModel] and
/// [PlaybackPresentationModel] for built-in view and controls.
///
/// Reactive 状态均为 [ReadonlySignal]，请在 [SignalBuilder] 或 `effect` 内读取 `.value`。
/// All reactive state is exposed as [ReadonlySignal]s; read `.value` inside [SignalBuilder] or `effect`.
///
/// 全屏 API（[isFullscreen]、[enterFullscreen]、[exitFullscreen]）须在 [XueHuaVideoView] 内
/// 通过 [attachImmersive] 绑定后生效，供顶栏插槽返回按钮先退出全屏再导航。
/// Fullscreen API requires [attachImmersive] from [XueHuaVideoView] before use.
class XueHuaPlayerController
    implements PlaybackControlsModel, PlaybackPresentationModel {
  /// 创建控制器；可注入测试用 [port]、[mediaSourceResolver] 或 [session] / Creates a controller with optional test doubles.
  XueHuaPlayerController({
    PlayerCommandPort? port,
    MediaSourceResolver? mediaSourceResolver,
    PlaybackSession? session,
  }) : _session =
           session ??
           PlaybackSession(
             port: port,
             mediaSourceResolver: mediaSourceResolver,
           );

  final PlaybackSession _session;

  ImmersiveControlsState? _immersive;
  Computed<bool>? _isFullscreenAttached;
  final FlutterSignal<bool> _isFullscreenDetached = signal(false);

  /// 是否已完成 [initialize]（原生 player 已创建且事件流已订阅）/ Whether [initialize] completed.
  @override
  ReadonlySignal<bool> get initialized => _session.initialized;

  /// 原生播放器 ID；[initialize] 后非 null，供 Texture 表面绑定 / Native player id; non-null after [initialize].
  @override
  ReadonlySignal<int?> get playerId => _session.playerId;

  /// 当前 [PlayerState] / Current [PlayerState].
  @override
  ReadonlySignal<PlayerState> get state => _session.state;

  /// 播放位置 / Playback position.
  @override
  ReadonlySignal<Duration> get position => _session.position;

  /// 媒体总时长 / Media duration.
  @override
  ReadonlySignal<Duration> get duration => _session.duration;

  /// 视频帧像素尺寸 / Video frame size in pixels.
  ReadonlySignal<Size> get videoSize => _session.videoSize;

  /// 缓冲进度 0–100 / Buffering progress 0–100.
  @override
  ReadonlySignal<int> get bufferingPercent => _session.bufferingPercent;

  /// 音量 0.0–1.0 / Volume 0.0–1.0.
  @override
  ReadonlySignal<double> get volume => _session.volume;

  /// 播放倍速 / Playback speed multiplier.
  @override
  ReadonlySignal<double> get speed => _session.speed;

  /// 是否循环播放 / Whether looping is enabled.
  @override
  ReadonlySignal<bool> get looping => _session.looping;

  /// 是否静音 / Whether audio is muted.
  @override
  ReadonlySignal<bool> get muted => _session.muted;

  /// 最近一次错误信息；无错误时为 null / Last error message, or null.
  ReadonlySignal<String?> get error => _session.error;

  /// 当前媒体音轨/视频轨/字幕轨列表 / Audio, video, and subtitle tracks for current media.
  ReadonlySignal<List<MediaTrack>> get tracks => _session.tracks;

  /// 视频元数据（含 DAR）；无视频轨时可能为 null / Video metadata including DAR; null when no video track.
  ReadonlySignal<VideoMetadata?> get videoMetadata => _session.videoMetadata;

  /// 当前 pipeline 是否支持 seek / Whether the active pipeline supports seeking.
  @override
  ReadonlySignal<bool> get isSeekable => _session.isSeekable;

  /// 当前 pipeline 是否支持多轨选择 / Whether multi-track selection is supported.
  ReadonlySignal<bool> get supportsTracks => _session.supportsTracks;

  /// 当前 pipeline 是否支持视频方向变换 / Whether video orientation transforms are supported.
  @override
  ReadonlySignal<bool> get supportsOrientation => _session.supportsOrientation;

  /// 当前视频顺时针旋转角度 / Current clockwise video rotation.
  @override
  ReadonlySignal<VideoRotation> get videoRotation => _session.videoRotation;

  /// 是否处于移动端全屏（横屏锁定）/ Whether mobile landscape fullscreen is active.
  ///
  /// 未 [attachImmersive] 或桌面端恒为 `false`。
  /// Always `false` on desktop or before [attachImmersive].
  ReadonlySignal<bool> get isFullscreen =>
      _isFullscreenAttached ?? _isFullscreenDetached;

  /// 是否正在播放（`state == playing`）/ Whether playback is active.
  @override
  ReadonlySignal<bool> get isPlaying => _session.isPlaying;

  /// 是否已播放到结尾 / Whether playback reached end-of-stream.
  ReadonlySignal<bool> get isCompleted => _session.isCompleted;

  /// 显示宽高比；优先 DAR，否则由 [videoSize] 推算 / Display aspect ratio from metadata or [videoSize].
  @override
  ReadonlySignal<double> get aspectRatio => _session.aspectRatio;

  /// 媒体打开代数；每次 [open] 递增 / Media open generation; increments on each [open].
  ReadonlySignal<int> get mediaGeneration => _session.mediaGeneration;

  /// 绑定 [XueHuaVideoView] 的沉浸状态，供全屏 API 读写横屏锁定 / Binds immersive state for fullscreen API.
  void attachImmersive(ImmersiveControlsState immersive) {
    detachImmersive();
    _immersive = immersive;
    _isFullscreenAttached = computed(() {
      if (!isMobilePlatform) return false;
      return immersive.landscapeLocked.value;
    });
  }

  /// 解除沉浸绑定 / Detaches immersive state.
  void detachImmersive() {
    _isFullscreenAttached?.dispose();
    _isFullscreenAttached = null;
    _immersive = null;
  }

  /// 进入移动端全屏（横屏锁定）/ Enters mobile landscape fullscreen.
  void enterFullscreen() {
    if (!isMobilePlatform) return;
    _immersive?.landscapeLocked.value = true;
  }

  /// 退出移动端全屏 / Exits mobile landscape fullscreen.
  void exitFullscreen() {
    if (!isMobilePlatform) return;
    _immersive?.landscapeLocked.value = false;
  }

  /// 创建原生 player 并订阅 Rust 事件流 / Creates the native player and subscribes to the Rust event stream.
  Future<void> initialize() => _session.initialize();

  /// 加载 [source]；可选 [autoPlay] 立即开始播放 / Loads [source]; optionally starts playback.
  Future<void> open(VideoSource source, {bool autoPlay = false}) =>
      _session.open(source, autoPlay: autoPlay);

  /// 开始或恢复播放 / Starts or resumes playback.
  Future<void> play() => _session.play();

  /// 暂停播放 / Pauses playback.
  Future<void> pause() => _session.pause();

  /// 停止播放并重置 transport / Stops playback and resets transport.
  Future<void> stop() => _session.stop();

  /// 播放中则暂停，否则播放 / Plays if paused, pauses if playing.
  @override
  Future<void> togglePlayPause() => _session.togglePlayPause();

  /// 跳转到 [position] / Seeks to [position].
  @override
  Future<void> seek(Duration position) => _session.seek(position);

  /// 设置音量 [volume]（0.0–1.0）/ Sets volume in 0.0–1.0.
  @override
  Future<void> setVolume(double volume) => _session.setVolume(volume);

  /// 设置静音 / Sets mute state.
  Future<void> setMuted(bool muted) => _session.setMuted(muted);

  /// 切换静音 / Toggles mute.
  @override
  Future<void> toggleMuted() => _session.toggleMuted();

  /// 设置播放倍速 / Sets playback speed.
  @override
  Future<void> setSpeed(double speed) => _session.setSpeed(speed);

  /// 设置循环播放 / Sets looping at end-of-stream.
  @override
  Future<void> setLooping(bool looping) => _session.setLooping(looping);

  /// 从 Rust 重新拉取轨道列表 / Refreshes the track list from Rust.
  Future<void> refreshTracks() => _session.refreshTracks();

  /// 选中或取消选中 [track] / Selects or deselects [track].
  Future<void> selectTrack(MediaTrack track, {bool enable = true}) =>
      _session.selectTrack(track, enable: enable);

  /// 设置视频顺时针旋转（需 [supportsOrientation]）/ Sets video rotation when [supportsOrientation].
  @override
  Future<void> setVideoRotation(VideoRotation rotation) =>
      _session.setVideoRotation(rotation);

  /// 设置宽高比缩放模式并同步至 GStreamer / Sets aspect ratio mode and syncs to GStreamer.
  @override
  Future<void> setAspectRatioMode(AspectRatioMode mode) =>
      _session.setAspectRatioMode(mode);

  /// 释放 player 与事件订阅 / Disposes the player and event subscription.
  Future<void> dispose() async {
    detachImmersive();
    _isFullscreenDetached.dispose();
    await _session.dispose();
  }
}
