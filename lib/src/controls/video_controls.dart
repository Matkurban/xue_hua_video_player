import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:signals/signals_flutter.dart';

import '../enum/video_controls_style.dart';
import '../domain/player_events.dart';
import '../theme/video_controls_theme.dart';
import '../utils/platform_util.dart';
import 'cupertino_video_controls.dart';
import 'video_controls_top_bar.dart';
import 'immersive_controls_state.dart';
import 'immersive_gesture_layer.dart';
import 'immersive_hud.dart';
import 'material_video_controls.dart';
import 'playback_controls_model.dart';

/// 在视频上方绘制自适应、自动隐藏控件栏的 overlay / Overlay with adaptive, auto-hiding control bar on top of the video.
///
/// 点击视频切换可见性；播放中无操作 [autoHide] 后自动隐藏。Reactive 读取均在 [SignalBuilder] 内，仅重建受影响控件。
/// Tap toggles visibility; hides after [autoHide] while playing. Reactive reads inside [SignalBuilder] for granular rebuilds.
class VideoControls extends StatefulWidget {
  /// 创建控件 overlay / Creates the controls overlay.
  ///
  /// # 参数 / Parameters
  /// - `model` — [PlaybackControlsModel] 实现 / playback controls model
  /// - `immersive` — 沉浸 signals 数据源 / immersive signals state
  /// - `style` — Material / Cupertino / adaptive
  /// - `autoHide` — 播放中自动隐藏延迟 / auto-hide delay while playing
  const VideoControls({
    super.key,
    required this.model,
    required this.immersive,
    this.style = VideoControlsStyle.adaptive,
    this.autoHide = const Duration(seconds: 3),
  });

  final PlaybackControlsModel model;
  final ImmersiveControlsState immersive;
  final VideoControlsStyle style;
  final Duration autoHide;

  @override
  State<VideoControls> createState() => _VideoControlsState();
}

class _VideoControlsState extends State<VideoControls> {
  final FlutterSignal<bool> _visible = signal(true);
  final FocusNode _focusNode = FocusNode();

  Timer? _hideTimer;

  late final void Function() _disposeStateEffect;

  bool _orientationLocked = false;

  late final void Function() _disposeOrientationEffect;

  List<DeviceOrientation>? _savedOrientations;

  @override
  void initState() {
    super.initState();
    _scheduleHide();
    _disposeStateEffect = effect(() {
      final state = widget.model.state.value;
      if (state == PlayerState.buffering || state == PlayerState.idle) {
        _visible.value = true;
        _scheduleHide();
      }
    });
    _disposeOrientationEffect = effect(() {
      final locked = widget.immersive.landscapeLocked.value;
      _orientationLocked = locked;
      if (locked) {
        SystemChrome.setPreferredOrientations([
          DeviceOrientation.landscapeLeft,
          DeviceOrientation.landscapeRight,
        ]);
        SystemChrome.setEnabledSystemUIMode(SystemUiMode.immersiveSticky);
      } else {
        if (_savedOrientations != null) {
          SystemChrome.setPreferredOrientations(_savedOrientations!);
        }
        SystemChrome.setEnabledSystemUIMode(SystemUiMode.edgeToEdge);
      }
    });
    if (isMobilePlatform) {
      // Flutter has no getter for current preferred orientations; restore all on exit.
      _savedOrientations = DeviceOrientation.values;
    }
  }

  @override
  void dispose() {
    _hideTimer?.cancel();
    _disposeStateEffect();
    _disposeOrientationEffect();
    _visible.dispose();
    _focusNode.dispose();
    if (_orientationLocked) {
      SystemChrome.setPreferredOrientations(
        _savedOrientations ?? DeviceOrientation.values,
      );
    }
    super.dispose();
  }

  void _scheduleHide() {
    _hideTimer?.cancel();
    _hideTimer = Timer(widget.autoHide, () {
      if (!mounted) return;
      if (widget.model.isPlaying.value) {
        _visible.value = false;
      }
    });
  }

  void _keepAlive() {
    if (!_visible.value) {
      _visible.value = true;
    }
    _scheduleHide();
  }

  void _toggle() {
    _visible.value = !_visible.value;
    if (_visible.value) _scheduleHide();
  }

  void _toggleFullscreen() {
    widget.immersive.landscapeLocked.value =
        !widget.immersive.landscapeLocked.value;
    _keepAlive();
  }

  KeyEventResult _onKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) return KeyEventResult.ignored;

    final step = widget.immersive.fullscreen.value.seekStep.duration;
    final position = widget.model.position.value;
    final duration = widget.model.duration.value;

    switch (event.logicalKey) {
      case LogicalKeyboardKey.space:
        unawaited(_togglePlayPauseWithHud());
        _keepAlive();
        return KeyEventResult.handled;
      case LogicalKeyboardKey.arrowLeft:
        unawaited(_seekBy(-step, position, duration));
        return KeyEventResult.handled;
      case LogicalKeyboardKey.arrowRight:
        unawaited(_seekBy(step, position, duration));
        return KeyEventResult.handled;
      case LogicalKeyboardKey.arrowUp:
        unawaited(_adjustVolume(0.05));
        return KeyEventResult.handled;
      case LogicalKeyboardKey.arrowDown:
        unawaited(_adjustVolume(-0.05));
        return KeyEventResult.handled;
      default:
        return KeyEventResult.ignored;
    }
  }

  Future<void> _togglePlayPauseWithHud() async {
    final wasPlaying = widget.model.isPlaying.value;
    await widget.model.togglePlayPause();
    widget.immersive.showHud(
      ImmersiveHudSnapshot(
        kind: ImmersiveHudKind.playPause,
        value: wasPlaying ? 0.0 : 1.0,
      ),
    );
  }

  Future<void> _seekBy(
    Duration delta,
    Duration position,
    Duration duration,
  ) async {
    final target = position + delta;
    final clamped = Duration(
      milliseconds: math.max(
        0,
        math.min(target.inMilliseconds, duration.inMilliseconds),
      ),
    );
    await widget.model.seek(clamped);
    widget.immersive.showHud(
      ImmersiveHudSnapshot(
        kind: ImmersiveHudKind.seek,
        value: delta.inSeconds.abs().toDouble(),
        forward: delta.inMilliseconds > 0,
      ),
    );
    _keepAlive();
  }

  Future<void> _adjustVolume(double delta) async {
    final volume = (widget.model.volume.value + delta).clamp(0.0, 1.0);
    await widget.model.setVolume(volume);
    widget.immersive.showHud(
      ImmersiveHudSnapshot(kind: ImmersiveHudKind.volume, value: volume),
    );
    _keepAlive();
  }

  bool _useCupertino(BuildContext context) {
    switch (widget.style) {
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

    final controls = cupertino
        ? CupertinoVideoControls(
            model: widget.model,
            theme: theme,
            onInteract: _keepAlive,
            showFullscreenButton: isMobilePlatform,
            landscapeLocked: widget.immersive.landscapeLocked,
            onFullscreenToggle: _toggleFullscreen,
            immersive: widget.immersive,
          )
        : MaterialVideoControls(
            model: widget.model,
            theme: theme,
            onInteract: _keepAlive,
            showFullscreenButton: isMobilePlatform,
            landscapeLocked: widget.immersive.landscapeLocked,
            onFullscreenToggle: _toggleFullscreen,
            immersive: widget.immersive,
          );

    final chrome = Stack(
      fit: StackFit.expand,
      children: [
        VideoControlsTopBar(
          immersive: widget.immersive,
          model: widget.model,
          theme: theme,
          slots: widget.immersive.fullscreen.value.overlaySlots,
          orientationLabels:
              widget.immersive.fullscreen.value.orientationLabels,
          showOrientationMenu:
              widget.immersive.fullscreen.value.showOrientationMenu,
        ),
        controls,
      ],
    );

    Widget body = Stack(
      fit: StackFit.expand,
      children: [
        Positioned.fill(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: _toggle,
            onDoubleTap: () async {
              final wasPlaying = widget.model.isPlaying.value;
              await widget.model.togglePlayPause();
              widget.immersive.showHud(
                ImmersiveHudSnapshot(
                  kind: ImmersiveHudKind.playPause,
                  value: wasPlaying ? 0.0 : 1.0,
                ),
              );
            },
            child: const SizedBox.expand(),
          ),
        ),
        SignalBuilder(
          builder: (context) {
            if (!widget.immersive.immersiveActive.value) {
              return const SizedBox.shrink();
            }
            return Stack(
              fit: StackFit.expand,
              children: [
                if (isMobilePlatform)
                  ImmersiveGestureLayer(
                    immersive: widget.immersive,
                    model: widget.model,
                    onTap: _toggle,
                  ),
              ],
            );
          },
        ),
        if (!isMobilePlatform)
          Focus(
            focusNode: _focusNode,
            autofocus: true,
            onKeyEvent: _onKeyEvent,
            // 仅捕获键盘，不阻挡点击切换控件栏 / Keys only; taps pass through.
            child: const IgnorePointer(child: SizedBox.expand()),
          ),
        SignalBuilder(
          builder: (context) {
            return AnimatedOpacity(
              key: const ValueKey('video-controls-opacity'),
              opacity: _visible.value ? 1 : 0,
              duration: const Duration(milliseconds: 200),
              child: IgnorePointer(
                ignoring: !_visible.value,
                child: chrome,
              ),
            );
          },
        ),
        ImmersiveHud(immersive: widget.immersive),
      ],
    );

    if (!isMobilePlatform) {
      body = MouseRegion(onHover: (_) => _keepAlive(), child: body);
    }

    return Positioned.fill(child: body);
  }
}
