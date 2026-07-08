import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../constant/constant.dart';
import '../theme/video_controls_theme.dart';
import '../utils/time_util.dart';
import 'center_button.dart';
import 'immersive_controls_state.dart';
import 'playback_controls_model.dart';
import 'playback_progress_slider.dart';
import 'scrub_controller.dart';

/// Material 风格内置视频控件栏 / Material-styled built-in video control bar.
///
/// 底部渐变 scrim、Material [Slider]、倍速 PopupMenu；中央 [CenterButton]。
/// Bottom gradient scrim, Material [Slider], speed PopupMenu; central [CenterButton].
class MaterialVideoControls extends StatefulWidget {
  const MaterialVideoControls({
    super.key,
    required this.model,
    required this.theme,
    required this.onInteract,
    this.showFullscreenButton = false,
    this.landscapeLocked,
    this.onFullscreenToggle,
    this.immersive,
  });

  final PlaybackControlsModel model;
  final VideoControlsTheme theme;
  final VoidCallback onInteract;

  /// 是否显示全屏按钮（仅移动端）/ Whether to show the fullscreen toggle (mobile only).
  final bool showFullscreenButton;

  /// 横屏锁定态 / Landscape lock state.
  final ReadonlySignal<bool>? landscapeLocked;

  /// 全屏切换回调 / Fullscreen toggle callback.
  final VoidCallback? onFullscreenToggle;

  /// 沉浸状态；用于中央按钮与 HUD 互斥 / Immersive state for center/HUD mutex.
  final ImmersiveControlsState? immersive;

  @override
  State<MaterialVideoControls> createState() => _MaterialVideoControlsState();
}

class _MaterialVideoControlsState extends State<MaterialVideoControls> {
  late final ScrubController _scrub;

  @override
  void initState() {
    super.initState();
    _scrub = ScrubController(
      model: widget.model,
      onInteract: widget.onInteract,
    );
  }

  @override
  void dispose() {
    _scrub.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final model = widget.model;
    final theme = widget.theme;
    return Stack(
      fit: StackFit.expand,
      children: [
        Align(
          alignment: Alignment.center,
          child: CenterButton(
            model: model,
            theme: theme,
            onInteract: widget.onInteract,
            hud: widget.immersive?.hud,
          ),
        ),
        Positioned(
          left: 0,
          right: 0,
          bottom: 0,
          child: SafeArea(
            top: false,
            child: Container(
              padding: theme.barPadding,
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment.bottomCenter,
                  end: Alignment.topCenter,
                  colors: [
                    theme.backgroundColor,
                    theme.backgroundColor.withValues(alpha: 0),
                  ],
                ),
              ),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  SliderTheme(
                    data: SliderTheme.of(context).copyWith(
                      activeTrackColor: theme.activeTrackColor,
                      inactiveTrackColor: theme.inactiveTrackColor,
                      thumbColor: theme.thumbColor,
                      secondaryActiveTrackColor: theme.bufferedTrackColor,
                      trackHeight: 3,
                      overlayShape: const RoundSliderOverlayShape(
                        overlayRadius: 12,
                      ),
                      thumbShape: const RoundSliderThumbShape(
                        enabledThumbRadius: 6,
                      ),
                    ),
                    child: PlaybackProgressSlider(
                      model: model,
                      scrub: _scrub,
                      builder: (context, snap) => Slider(
                        value: snap.displayValue,
                        onChangeStart: snap.enabled
                            ? (_) => snap.onSeekStart?.call()
                            : null,
                        onChanged: snap.onSeekChanged,
                        onChangeEnd: snap.onSeekEnd,
                      ),
                    ),
                  ),
                  Row(
                    children: [
                      SignalBuilder(
                        builder: (context) => Text(
                          '${formatDuration(model.position.value)} / ${formatDuration(model.duration.value)}',
                          style: TextStyle(
                            color: theme.textColor,
                            fontSize: 12,
                          ),
                        ),
                      ),
                      const Spacer(),
                      SignalBuilder(
                        builder: (context) => IconButton(
                          style: IconButton.styleFrom(
                            tapTargetSize: .shrinkWrap,
                            visualDensity: .compact,
                          ),
                          color: theme.iconColor,
                          icon: Icon(
                            model.muted.value || model.volume.value == 0
                                ? Icons.volume_off
                                : Icons.volume_up,
                            size: theme.secondaryIconSize,
                          ),
                          onPressed: () {
                            widget.onInteract();
                            model.toggleMuted();
                          },
                        ),
                      ),
                      SignalBuilder(
                        builder: (context) => IconButton(
                          style: IconButton.styleFrom(
                            tapTargetSize: .shrinkWrap,
                            visualDensity: .compact,
                          ),
                          color: model.looping.value
                              ? theme.iconColor
                              : theme.iconColor.withValues(alpha: 0.5),
                          icon: Icon(Icons.loop, size: theme.secondaryIconSize),
                          onPressed: () async {
                            widget.onInteract();
                            await model.setLooping(!model.looping.value);
                          },
                        ),
                      ),
                      SignalBuilder(
                        builder: (context) => PopupMenuButton<double>(
                          tooltip: 'Playback speed',
                          initialValue: model.speed.value,
                          onSelected: (v) {
                            widget.onInteract();
                            model.setSpeed(v);
                          },
                          itemBuilder: (context) => [
                            for (final s in speeds)
                              PopupMenuItem<double>(
                                value: s,
                                child: Text('${s}x'),
                              ),
                          ],
                          child: Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 8),
                            child: Text(
                              '${model.speed.value}x',
                              style: TextStyle(
                                color: theme.iconColor,
                                fontSize: theme.secondaryIconSize * 0.7,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                          ),
                        ),
                      ),
                      if (widget.showFullscreenButton &&
                          widget.landscapeLocked != null &&
                          widget.onFullscreenToggle != null)
                        SignalBuilder(
                          builder: (context) => IconButton(
                            style: IconButton.styleFrom(
                              tapTargetSize: .shrinkWrap,
                              visualDensity: .compact,
                            ),
                            color: theme.iconColor,
                            icon: Icon(
                              widget.landscapeLocked!.value
                                  ? Icons.fullscreen_exit
                                  : Icons.fullscreen,
                              size: theme.secondaryIconSize,
                            ),
                            onPressed: () {
                              widget.onInteract();
                              widget.onFullscreenToggle!();
                            },
                          ),
                        ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}
