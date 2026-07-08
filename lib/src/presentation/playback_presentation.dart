import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../controls/buffering_indicator.dart';
import '../enum/video_controls_style.dart';
import '../rust/api/types.dart';
import '../rust/player_events.dart';
import '../surface/texture_surface.dart';
import '../surface/video_surface_handle.dart';
import '../theme/video_controls_theme.dart';
import 'playback_presentation_model.dart';

/// 深度呈现模块：平台表面嵌入、宽高比布局、缓冲 UI / Deep presentation: platform surface embed, aspect layout, buffering chrome.
///
/// 根据 [model.playerId] 路由至 [TextureVideoSurface]，并用 [SignalBuilder] 响应 [aspectRatio] 变化。
/// Routes to [TextureVideoSurface] from [model.playerId] and reacts to [aspectRatio] via [SignalBuilder].
class PlaybackPresentation extends StatelessWidget {
  /// 创建呈现层 / Creates the presentation layer.
  ///
  /// # 参数 / Parameters
  /// - `model` — 实现 [PlaybackPresentationModel] 的控制器 / controller implementing the model
  /// - `aspectRatioMode` — fit / fill / stretch 布局策略 signal / layout strategy signal
  /// - `controlsStyle` — 缓冲指示器 Material/Cupertino 风格 / buffering indicator style
  const PlaybackPresentation({
    super.key,
    required this.model,
    required this.aspectRatioMode,
    this.controlsStyle = VideoControlsStyle.adaptive,
  });

  final PlaybackPresentationModel model;

  /// letterbox / 裁剪 / 拉伸；Texture 在 Dart 布局；Android 亦转发至 `glimagesink` / Letterbox, crop, or stretch; Dart layout plus Android sink forward.
  final ReadonlySignal<AspectRatioMode> aspectRatioMode;

  /// 缓冲指示器视觉风格 / Visual style for the buffering indicator.
  final VideoControlsStyle controlsStyle;

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        final playerId = model.playerId.value;
        if (playerId == null) return const SizedBox.shrink();
        final handle = VideoSurfaceHandle.fromPlayerId(playerId);
        final ratio = model.aspectRatio.value;

        return SignalBuilder(
          builder: (context) {
            final mode = aspectRatioMode.value;
            return Stack(
              fit: StackFit.expand,
              children: [
                _AspectRatioModeSync(
                  model: model,
                  aspectRatioMode: mode,
                  child: _VideoAspectLayout(
                    aspectRatio: ratio,
                    mode: mode,
                    child: TextureVideoSurface(handle: handle),
                  ),
                ),
                _BufferingOverlay(
                  model: model,
                  controlsStyle: controlsStyle,
                ),
              ],
            );
          },
        );
      },
    );
  }
}

/// 为外部 [Texture] 渲染计算视频 widget 尺寸 / Sizes the video widget for external [Texture] rendering.
///
/// 三种 mode 共用同一 widget 树，仅 [BoxFit] 不同，避免 reparent Texture 子树。
/// All modes share one widget tree; only [BoxFit] changes to avoid reparenting the Texture subtree.
class _VideoAspectLayout extends StatelessWidget {
  const _VideoAspectLayout({
    required this.aspectRatio,
    required this.mode,
    required this.child,
  });

  final double aspectRatio;
  final AspectRatioMode mode;
  final Widget child;

  static BoxFit _boxFitForMode(AspectRatioMode mode) {
    return switch (mode) {
      AspectRatioMode.fit => BoxFit.contain,
      AspectRatioMode.fill => BoxFit.cover,
      AspectRatioMode.stretch => BoxFit.fill,
    };
  }

  @override
  Widget build(BuildContext context) {
    final ratio = aspectRatio > 0 ? aspectRatio : 16 / 9;

    return SizedBox.expand(
      child: FittedBox(
        fit: _boxFitForMode(mode),
        alignment: Alignment.center,
        clipBehavior: Clip.hardEdge,
        child: SizedBox(
          width: ratio,
          height: 1,
          child: child,
        ),
      ),
    );
  }
}

/// 缓冲中主题化指示器 / Themed buffering indicator while loading or rebuffering.
class _BufferingOverlay extends StatelessWidget {
  const _BufferingOverlay({
    required this.model,
    required this.controlsStyle,
  });

  final PlaybackPresentationModel model;
  final VideoControlsStyle controlsStyle;

  bool _useCupertino(BuildContext context) {
    switch (controlsStyle) {
      case VideoControlsStyle.material:
        return false;
      case VideoControlsStyle.cupertino:
        return true;
      case VideoControlsStyle.adaptive:
        final platform = Theme.of(context).platform;
        return platform == TargetPlatform.iOS ||
            platform == TargetPlatform.macOS;
    }
  }

  @override
  Widget build(BuildContext context) {
    final cupertino = _useCupertino(context);
    final theme =
        Theme.of(context).extension<VideoControlsTheme>() ??
        (cupertino
            ? VideoControlsTheme.cupertino()
            : VideoControlsTheme.material());

    return SignalBuilder(
      builder: (context) {
        final buffering = model.bufferingPercent.value;
        final state = model.state.value;
        if (buffering >= 100 && state != PlayerState.buffering) {
          return const SizedBox.shrink();
        }
        return BufferingIndicator(
          bufferingPercent: buffering,
          theme: theme,
          cupertino: cupertino,
        );
      },
    );
  }
}

/// 播放器就绪后将 [aspectRatioMode] 推送至 Rust pipeline / Pushes [aspectRatioMode] to Rust when the player is ready.
class _AspectRatioModeSync extends StatefulWidget {
  const _AspectRatioModeSync({
    required this.model,
    required this.aspectRatioMode,
    required this.child,
  });

  final PlaybackPresentationModel model;
  final AspectRatioMode aspectRatioMode;
  final Widget child;

  @override
  State<_AspectRatioModeSync> createState() => _AspectRatioModeSyncState();
}

class _AspectRatioModeSyncState extends State<_AspectRatioModeSync> {
  @override
  void initState() {
    super.initState();
    _sync();
  }

  @override
  void didUpdateWidget(covariant _AspectRatioModeSync oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.aspectRatioMode != widget.aspectRatioMode ||
        oldWidget.model != widget.model) {
      _sync();
    }
  }

  void _sync() {
    if (widget.model.playerId.value == null) return;
    if (!widget.model.initialized.value) return;
    widget.model.setAspectRatioMode(widget.aspectRatioMode);
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
