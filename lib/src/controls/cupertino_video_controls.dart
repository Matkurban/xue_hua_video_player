import 'dart:ui';

import 'package:chat_context_menu/chat_context_menu.dart';
import 'package:flutter/cupertino.dart';
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

/// Cupertino 风格内置视频控件栏 / Cupertino-styled built-in video control bar.
///
/// 磨砂底栏、[CupertinoSlider]、倍速 ActionSheet；中央 [CenterButton]。
/// Frosted bar, [CupertinoSlider], speed ActionSheet; central [CenterButton].
class CupertinoVideoControls extends StatefulWidget {
  const CupertinoVideoControls({
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
  State<CupertinoVideoControls> createState() => _CupertinoVideoControlsState();
}

class _CupertinoVideoControlsState extends State<CupertinoVideoControls> {
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
    final size = MediaQuery.sizeOf(context);
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
          left: 8,
          right: 8,
          bottom: 8,
          child: SafeArea(
            top: false,
            child: ClipRRect(
              borderRadius: BorderRadius.circular(theme.borderRadius),
              child: BackdropFilter(
                filter: ImageFilter.blur(sigmaX: 12, sigmaY: 12),
                child: DecoratedBox(
                  decoration: BoxDecoration(color: theme.backgroundColor),
                  child: SizedBox(
                    width: size.width - 16,
                    child: Padding(
                      padding: theme.barPadding,
                      child: Row(
                        children: [
                          SignalBuilder(
                            builder: (context) => IconButton(
                              onPressed: () {
                                widget.onInteract();
                                model.toggleMuted();
                              },
                              style: IconButton.styleFrom(
                                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                                visualDensity: VisualDensity.compact,
                              ),
                              icon: Icon(
                                model.muted.value || model.volume.value == 0
                                    ? CupertinoIcons.volume_off
                                    : CupertinoIcons.volume_up,
                                size: theme.secondaryIconSize,
                                color: theme.iconColor,
                              ),
                            ),
                          ),
                          const SizedBox(width: 10),
                          SignalBuilder(
                            builder: (context) => Text(
                              formatDuration(model.position.value),
                              style: TextStyle(
                                color: theme.textColor,
                                fontSize: 12,
                              ),
                            ),
                          ),
                          Expanded(
                            child: Padding(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 8,
                              ),
                              child: PlaybackProgressSlider(
                                model: model,
                                scrub: _scrub,
                                builder: (context, snap) => CupertinoSlider(
                                  value: snap.displayValue,
                                  activeColor: theme.activeTrackColor,
                                  thumbColor: theme.thumbColor,
                                  onChangeStart: snap.enabled
                                      ? (_) => snap.onSeekStart?.call()
                                      : null,
                                  onChanged: snap.onSeekChanged,
                                  onChangeEnd: snap.onSeekEnd,
                                ),
                              ),
                            ),
                          ),
                          SignalBuilder(
                            builder: (context) => Text(
                              formatDuration(model.duration.value),
                              style: TextStyle(
                                color: theme.textColor,
                                fontSize: 12,
                              ),
                            ),
                          ),
                          const SizedBox(width: 10),
                          SignalBuilder(
                            builder: (context) => IconButton(
                              onPressed: () async {
                                widget.onInteract();
                                await model.setLooping(!model.looping.value);
                              },
                              style: IconButton.styleFrom(
                                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                                visualDensity: VisualDensity.compact,
                              ),
                              icon: Icon(
                                CupertinoIcons.repeat,
                                size: theme.secondaryIconSize,
                                color: model.looping.value
                                    ? theme.iconColor
                                    : theme.iconColor.withValues(alpha: 0.5),
                              ),
                            ),
                          ),
                          const SizedBox(width: 10),
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
                                            padding: const EdgeInsets.symmetric(
                                              horizontal: 16,
                                              vertical: 10,
                                            ),
                                            child: Row(
                                              mainAxisSize: MainAxisSize.min,
                                              children: [
                                                SizedBox(
                                                  width: 20,
                                                  child: s == current
                                                      ? Icon(
                                                          Icons.check,
                                                          size: 16,
                                                          color:
                                                              theme.textColor,
                                                        )
                                                      : null,
                                                ),
                                                Text(
                                                  '${s}x',
                                                  style: TextStyle(
                                                    color: theme.textColor,
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
                                child: SignalBuilder(
                                  builder: (context) => Text(
                                    '${model.speed.value}x',
                                    style: TextStyle(
                                      color: theme.iconColor,
                                      fontSize: theme.secondaryIconSize * 0.7,
                                      fontWeight: FontWeight.w600,
                                    ),
                                  ),
                                ),
                              );
                            },
                          ),
                          if (widget.showFullscreenButton &&
                              widget.landscapeLocked != null &&
                              widget.onFullscreenToggle != null) ...[
                            const SizedBox(width: 10),
                            SignalBuilder(
                              builder: (context) => IconButton(
                                onPressed: () {
                                  widget.onInteract();
                                  widget.onFullscreenToggle!();
                                },
                                style: IconButton.styleFrom(
                                  tapTargetSize:
                                      MaterialTapTargetSize.shrinkWrap,
                                  visualDensity: VisualDensity.compact,
                                ),
                                icon: Icon(
                                  widget.landscapeLocked!.value
                                      ? CupertinoIcons
                                            .arrow_down_right_arrow_up_left
                                      : CupertinoIcons
                                            .arrow_up_left_arrow_down_right,
                                  size: theme.secondaryIconSize,
                                  color: theme.iconColor,
                                ),
                              ),
                            ),
                          ],
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
