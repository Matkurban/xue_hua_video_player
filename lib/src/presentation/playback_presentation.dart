import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../rust/api/types.dart';
import '../rust/player_events.dart';
import '../surface/build_video_surface.dart';
import '../surface/video_surface_handle.dart';
import 'playback_presentation_model.dart';

/// 深度呈现模块：平台表面嵌入、宽高比布局、缓冲 UI / Deep presentation: platform surface embed, aspect layout, buffering chrome.
///
/// 根据 [model.playerId] 路由至 [buildVideoSurface]，并用 [SignalBuilder] 响应 [aspectRatio] 变化。
/// Routes to [buildVideoSurface] from [model.playerId] and reacts to [aspectRatio] via [SignalBuilder].
class PlaybackPresentation extends StatelessWidget {
  /// 创建呈现层 / Creates the presentation layer.
  ///
  /// # 参数 / Parameters
  /// - `model` — 实现 [PlaybackPresentationModel] 的控制器 / controller implementing the model
  /// - `aspectRatioMode` — fit / fill / stretch 布局策略 / layout strategy
  const PlaybackPresentation({
    super.key,
    required this.model,
    required this.aspectRatioMode,
  });

  final PlaybackPresentationModel model;

  /// letterbox / 裁剪 / 拉伸；Texture 在 Dart 布局；Android 亦转发至 `glimagesink` / Letterbox, crop, or stretch; Dart layout plus Android sink forward.
  final AspectRatioMode aspectRatioMode;

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        final playerId = model.playerId.value;
        if (playerId == null) return const SizedBox.shrink();
        final handle = VideoSurfaceHandle.fromPlayerId(playerId);
        final ratio = model.aspectRatio.value;
        final surface = Stack(
          fit: StackFit.expand,
          children: [
            buildVideoSurface(handle),
            _BufferingOverlay(model: model),
          ],
        );
        return _AspectRatioModeSync(
          model: model,
          aspectRatioMode: aspectRatioMode,
          child: _VideoAspectLayout(
            aspectRatio: ratio,
            mode: aspectRatioMode,
            child: surface,
          ),
        );
      },
    );
  }
}

/// 为外部 [Texture] 渲染计算视频 widget 尺寸 / Sizes the video widget for external [Texture] rendering.
///
/// GStreamer `appsink` 帧为原始矩形；Flutter [Texture] 会拉伸至 widget 边界，故须在此保留 DAR。
/// GStreamer `appsink` frames are raw rectangles; Flutter [Texture] stretches to bounds, so DAR is preserved here.
class _VideoAspectLayout extends StatelessWidget {
  const _VideoAspectLayout({
    required this.aspectRatio,
    required this.mode,
    required this.child,
  });

  final double aspectRatio;
  final AspectRatioMode mode;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final ratio = aspectRatio > 0 ? aspectRatio : 16 / 9;

    switch (mode) {
      case AspectRatioMode.fit:
        return Center(
          child: AspectRatio(aspectRatio: ratio, child: child),
        );
      case AspectRatioMode.fill:
        return Center(
          child: AspectRatio(
            aspectRatio: ratio,
            child: FittedBox(
              fit: BoxFit.cover,
              alignment: Alignment.center,
              clipBehavior: Clip.hardEdge,
              child: SizedBox(width: ratio, height: 1, child: child),
            ),
          ),
        );
      case AspectRatioMode.stretch:
        return SizedBox.expand(child: child);
    }
  }
}

/// 缓冲中半透明遮罩与进度指示 / Semi-transparent overlay and progress indicator while buffering.
class _BufferingOverlay extends StatelessWidget {
  const _BufferingOverlay({required this.model});

  final PlaybackPresentationModel model;

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        final buffering = model.bufferingPercent.value;
        final state = model.state.value;
        if (buffering >= 100 && state != PlayerState.buffering) {
          return const SizedBox.shrink();
        }
        return ColoredBox(
          color: Colors.black38,
          child: Center(
            child: CircularProgressIndicator(
              value: buffering < 100 ? buffering / 100 : null,
            ),
          ),
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
