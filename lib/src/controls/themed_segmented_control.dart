import 'package:flutter/material.dart';

import '../theme/video_controls_theme.dart';

/// 主题化分段选择器行 / Themed segmented control row for video control panels.
///
/// 可选标题 + 全宽 [SegmentedButton]；样式取自 [VideoControlsTheme]。
/// Optional label row plus full-width [SegmentedButton]; colors from [VideoControlsTheme].
class ThemedSegmentedControl<T> extends StatelessWidget {
  /// 创建主题化分段选择器 / Creates a themed segmented control.
  const ThemedSegmentedControl({
    super.key,
    required this.segments,
    required this.selected,
    required this.onSelectionChanged,
    required this.theme,
    this.label,
    this.showSelectedIcon = false,
  });

  /// 分段定义 / Button segments.
  final List<ButtonSegment<T>> segments;

  /// 当前选中集合（单选时为单元素集）/ Current selection (singleton for single-select).
  final Set<T> selected;

  /// 选中变化回调 / Selection changed callback.
  final ValueChanged<Set<T>> onSelectionChanged;

  /// 控件主题 / Controls theme.
  final VideoControlsTheme theme;

  /// 可选标题文案 / Optional label above the control.
  final String? label;

  /// 是否在选中段显示勾选图标 / Whether to show a check icon on the selected segment.
  final bool showSelectedIcon;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (label != null) ...[
          Text(label!, style: TextStyle(color: theme.textColor, fontSize: 13)),
          const SizedBox(height: 6),
        ],
        SegmentedButton<T>(
          segments: segments,
          selected: selected,
          showSelectedIcon: showSelectedIcon,
          style: SegmentedButton.styleFrom(
            foregroundColor: theme.textColor,
            selectedForegroundColor: theme.textColor,
            backgroundColor: theme.backgroundColor.withValues(alpha: 0.5),
            selectedBackgroundColor: theme.activeTrackColor.withValues(
              alpha: 0.35,
            ),
            side: BorderSide(color: theme.textColor.withValues(alpha: 0.25)),
            visualDensity: VisualDensity.compact,
          ),
          onSelectionChanged: (value) {
            if (value.isEmpty) return;
            onSelectionChanged(value);
          },
        ),
      ],
    );
  }
}
