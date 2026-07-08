import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import 'controls/fullscreen_config.dart';
import 'controls/immersive_controls_state.dart';
import 'controls/video_controls.dart';
import 'enum/video_controls_style.dart';
import 'presentation/playback_presentation.dart';
import 'xue_hua_player_controller.dart';

export 'controls/fullscreen_config.dart';
export 'controls/video_controls.dart';

/// 通过 Flutter 外部 Texture 渲染 GStreamer 视频，可选内置自适应控件栏 / Renders GStreamer video through a Flutter external texture with an optional adaptive control bar.
///
/// 组合 [PlaybackPresentation]（视频表面 + 宽高比 + 缓冲指示）与 [VideoControls]（自动隐藏控件栏）。
/// Composes [PlaybackPresentation] (surface, aspect ratio, buffering chrome) and [VideoControls] (auto-hiding bar).
class XueHuaVideoView extends StatefulWidget {
  /// 创建视频视图 / Creates a video view.
  ///
  /// # 参数 / Parameters
  /// - `controller` — 已 [XueHuaPlayerController.initialize] 的控制器 / initialized controller
  /// - `aspectRatioMode` —  letterbox / 裁剪 / 拉伸，默认 [AspectRatioMode.fit] / letterbox, crop, or stretch
  /// - `backgroundColor` —  letterbox 区域背景色 / background behind letterbox bars
  /// - `showControls` — 是否叠加内置控件栏 / whether to overlay built-in controls
  /// - `controlsStyle` — 控件视觉风格 / control bar visual style
  /// - `fullscreen` — 沉浸控件配置 / immersive controls configuration
  const XueHuaVideoView({
    super.key,
    required this.controller,
    this.aspectRatioMode = AspectRatioMode.fit,
    this.backgroundColor = const Color(0xFF000000),
    this.showControls = true,
    this.controlsStyle = VideoControlsStyle.adaptive,
    this.fullscreen = const VideoControlsFullscreenConfig(),
  });

  /// 绑定的播放器控制器；同时作为 presentation 与 controls 的 model / Bound player controller; model for presentation and controls.
  final XueHuaPlayerController controller;

  /// 视频表面宽高比模式初始值 / Initial aspect ratio mode for the video surface.
  final AspectRatioMode aspectRatioMode;

  /// 视频周围/letterbox 区域背景色 / Color painted behind and around the video.
  final Color backgroundColor;

  /// 是否显示内置控件栏 / Whether to overlay the built-in control bar.
  final bool showControls;

  /// 内置控件栏风格（默认 adaptive）/ Built-in control bar style (default adaptive).
  final VideoControlsStyle controlsStyle;

  /// 全屏沉浸控件配置 / Fullscreen immersive controls configuration.
  final VideoControlsFullscreenConfig fullscreen;

  @override
  State<XueHuaVideoView> createState() => _XueHuaVideoViewState();
}

class _XueHuaVideoViewState extends State<XueHuaVideoView> {
  late final ImmersiveControlsState _immersive;
  late final void Function() _disposeOpenEffect;
  int _lastMediaGeneration = -1;

  @override
  void initState() {
    super.initState();
    _immersive = ImmersiveControlsState(
      initialAspectRatioMode: widget.aspectRatioMode,
      fullscreen: widget.fullscreen,
    );
    widget.controller.attachImmersive(_immersive);
    _disposeOpenEffect = effect(() {
      final generation = widget.controller.mediaGeneration.value;
      if (generation == _lastMediaGeneration) return;
      _lastMediaGeneration = generation;
      if (generation > 0) {
        _immersive.aspectRatioMode.value = AspectRatioMode.fit;
      }
    });
  }

  @override
  void didUpdateWidget(covariant XueHuaVideoView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.aspectRatioMode != widget.aspectRatioMode) {
      _immersive.aspectRatioMode.value = widget.aspectRatioMode;
    }
    if (oldWidget.fullscreen != widget.fullscreen) {
      _immersive.fullscreen.value = widget.fullscreen;
    }
  }

  @override
  void dispose() {
    _disposeOpenEffect();
    widget.controller.detachImmersive();
    _immersive.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Material(
      type: MaterialType.transparency,
      child: ColoredBox(
        color: widget.backgroundColor,
        child: Stack(
          fit: StackFit.expand,
          alignment: Alignment.center,
          children: [
            PlaybackPresentation(
              model: widget.controller,
              aspectRatioMode: _immersive.aspectRatioMode,
              controlsStyle: widget.controlsStyle,
            ),
            if (widget.showControls)
              VideoControls(
                model: widget.controller,
                immersive: _immersive,
                style: widget.controlsStyle,
              ),
          ],
        ),
      ),
    );
  }
}
