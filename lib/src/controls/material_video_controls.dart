import 'package:chat_context_menu/chat_context_menu.dart';
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
          child: Container(
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
            child: SafeArea(
              top: false,
              left: false,
              right: false,
              child: Padding(
                padding: theme.barPadding,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    SliderTheme(
                      data: SliderTheme.of(context).copyWith(
                        activeTrackColor: theme.activeTrackColor,
                        inactiveTrackColor: theme.inactiveTrackColor,
                        thumbColor: theme.thumbColor,
                        secondaryActiveTrackColor: theme.bufferedTrackColor,
                        trackHeight: 4,
                        overlayShape: const RoundSliderOverlayShape(
                          overlayRadius: 4,
                        ),
                        thumbShape: const RoundSliderThumbShape(
                          enabledThumbRadius: 4,
                        ),
                        padding: const EdgeInsets.symmetric(horizontal: 12),
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
                    LayoutBuilder(
                      builder: (context, constraints) {
                        final compact = constraints.maxWidth < 280;
                        return Padding(
                          padding: const EdgeInsets.only(left: 12, right: 12),
                          child: Row(
                            children: [
                              if (compact)
                                Flexible(
                                  child: SignalBuilder(
                                    builder: (context) => FittedBox(
                                      fit: BoxFit.scaleDown,
                                      alignment: Alignment.centerLeft,
                                      child: Text(
                                        '${formatDuration(model.position.value)} / ${formatDuration(model.duration.value)}',
                                        maxLines: 1,
                                        softWrap: false,
                                        style: TextStyle(
                                          color: theme.textColor,
                                          fontSize: 12,
                                        ),
                                      ),
                                    ),
                                  ),
                                )
                              else
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
                                    tapTargetSize:
                                        MaterialTapTargetSize.shrinkWrap,
                                    visualDensity: VisualDensity.compact,
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
                              if (!compact) ...[
                                SignalBuilder(
                                  builder: (context) => IconButton(
                                    style: IconButton.styleFrom(
                                      tapTargetSize:
                                          MaterialTapTargetSize.shrinkWrap,
                                      visualDensity: VisualDensity.compact,
                                    ),
                                    color: model.looping.value
                                        ? theme.iconColor
                                        : theme.iconColor.withValues(
                                            alpha: 0.5,
                                          ),
                                    icon: Icon(
                                      Icons.loop,
                                      size: theme.secondaryIconSize,
                                    ),
                                    onPressed: () async {
                                      widget.onInteract();
                                      await model.setLooping(
                                        !model.looping.value,
                                      );
                                    },
                                  ),
                                ),
                                ChatContextMenuWrapper(
                                  topPadding: 0,
                                  backgroundColor: theme.backgroundColor,
                                  borderRadius: BorderRadius.circular(
                                    theme.borderRadius,
                                  ),
                                  padding: EdgeInsets.zero,
                                  menuBuilder: (context, hideMenu) {
                                    return SignalBuilder(
                                      builder: (context) {
                                        final current = model.speed.value;
                                        return Column(
                                          mainAxisSize: MainAxisSize.min,
                                          children: [
                                            for (final s in speeds)
                                              InkWell(
                                                onTap: () {
                                                  widget.onInteract();
                                                  model.setSpeed(s);
                                                  hideMenu();
                                                },
                                                child: Padding(
                                                  padding:
                                                      const EdgeInsets.symmetric(
                                                        horizontal: 16,
                                                        vertical: 10,
                                                      ),
                                                  child: Row(
                                                    mainAxisSize:
                                                        MainAxisSize.min,
                                                    children: [
                                                      SizedBox(
                                                        width: 20,
                                                        child: s == current
                                                            ? Icon(
                                                                Icons.check,
                                                                size: 16,
                                                                color: theme
                                                                    .textColor,
                                                              )
                                                            : null,
                                                      ),
                                                      Text(
                                                        '${s}x',
                                                        style: TextStyle(
                                                          color:
                                                              theme.textColor,
                                                          fontSize: 14,
                                                        ),
                                                      ),
                                                    ],
                                                  ),
                                                ),
                                              ),
                                          ],
                                        );
                                      },
                                    );
                                  },
                                  widgetBuilder: (context, showMenu, _) {
                                    return GestureDetector(
                                      onTap: showMenu,
                                      child: Padding(
                                        padding: const EdgeInsets.symmetric(
                                          horizontal: 8,
                                        ),
                                        child: Row(
                                          mainAxisSize: MainAxisSize.min,
                                          children: [
                                            Icon(
                                              Icons.speed,
                                              size: theme.secondaryIconSize,
                                              color: theme.iconColor,
                                            ),
                                            const SizedBox(width: 4),
                                            SignalBuilder(
                                              builder: (context) => Text(
                                                '${model.speed.value}x',
                                                style: TextStyle(
                                                  color: theme.iconColor,
                                                  fontSize:
                                                      theme.secondaryIconSize *
                                                      0.7,
                                                  fontWeight: FontWeight.w600,
                                                ),
                                              ),
                                            ),
                                          ],
                                        ),
                                      ),
                                    );
                                  },
                                ),
                              ],
                              if (widget.showFullscreenButton &&
                                  widget.landscapeLocked != null &&
                                  widget.onFullscreenToggle != null)
                                SignalBuilder(
                                  builder: (context) => IconButton(
                                    style: IconButton.styleFrom(
                                      tapTargetSize:
                                          MaterialTapTargetSize.shrinkWrap,
                                      visualDensity: VisualDensity.compact,
                                      padding: EdgeInsets.zero,
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
                        );
                      },
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
