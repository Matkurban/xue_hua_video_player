import 'package:signals/signals_flutter.dart';

import '../rust/player_events.dart';

/// 视频呈现窄接口：表面路由、布局宽高比、加载指示 / Narrow seam for video presentation: surface routing, layout aspect, loading chrome.
///
/// 由 [XueHuaPlayerController] 实现；[PlaybackPresentation] 与 [VideoControls] 分别依赖此接口与 [PlaybackControlsModel]。
/// Implemented by [XueHuaPlayerController]; [PlaybackPresentation] and [VideoControls] depend on this and [PlaybackControlsModel] respectively.
abstract class PlaybackPresentationModel {
  /// 播放器是否已初始化 / Whether the player is initialized.
  ReadonlySignal<bool> get initialized;

  /// 原生 player ID；null 时不渲染表面 / Native player id; no surface when null.
  ReadonlySignal<int?> get playerId;

  /// 当前显示宽高比 / Current display aspect ratio.
  ReadonlySignal<double> get aspectRatio;

  /// 播放状态 / Playback state.
  ReadonlySignal<PlayerState> get state;

  /// 缓冲进度 0–100 / Buffering percent 0–100.
  ReadonlySignal<int> get bufferingPercent;

  /// 同步宽高比模式至 Rust pipeline / Syncs aspect ratio mode to the Rust pipeline.
  Future<void> setAspectRatioMode(AspectRatioMode mode);
}
