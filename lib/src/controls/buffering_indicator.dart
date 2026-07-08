import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';

import '../theme/video_controls_theme.dart';

/// 主题化缓冲指示器 / Themed buffering indicator (Material or Cupertino).
class BufferingIndicator extends StatelessWidget {
  /// 创建缓冲指示器 / Creates a buffering indicator.
  const BufferingIndicator({
    super.key,
    required this.bufferingPercent,
    required this.theme,
    required this.cupertino,
  });

  /// Material 进度圈物理尺寸 / Material spinner box size in logical pixels.
  static const double materialIndicatorSize = 36;

  /// Cupertino 进度圈物理尺寸 / Cupertino spinner box size in logical pixels.
  static const double cupertinoIndicatorSize = 20;

  /// 缓冲进度 0–100 / Buffering progress 0–100.
  final int bufferingPercent;

  /// 控件主题 / Controls theme.
  final VideoControlsTheme theme;

  /// 是否使用 Cupertino 风格 / Whether to use Cupertino styling.
  final bool cupertino;

  @override
  Widget build(BuildContext context) {
    final content = Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (cupertino)
          SizedBox(
            width: cupertinoIndicatorSize,
            height: cupertinoIndicatorSize,
            child: CupertinoActivityIndicator(
              radius: 10,
              color: theme.bufferingIndicatorColor,
            ),
          )
        else
          SizedBox(
            width: materialIndicatorSize,
            height: materialIndicatorSize,
            child: CircularProgressIndicator(
              value: bufferingPercent < 100 ? bufferingPercent / 100 : null,
              color: theme.bufferingIndicatorColor,
            ),
          ),
        const SizedBox(height: 8),
        Text(
          '$bufferingPercent%',
          style:
              theme.bufferingTextStyle ??
              TextStyle(color: theme.bufferingTextColor, fontSize: 12),
        ),
      ],
    );

    final centered = SizedBox.expand(child: Center(child: content));

    if (theme.bufferingScrimColor.a <= 0) {
      return centered;
    }

    return ColoredBox(
      color: theme.bufferingScrimColor,
      child: centered,
    );
  }
}
