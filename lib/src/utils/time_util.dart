/// 将 [Duration] 格式化为 `mm:ss` 或 `h:mm:ss` / Formats [Duration] as `mm:ss` or `h:mm:ss`.
///
/// 负值前缀 `-`；小时为 0 时省略小时段。
/// Negative values get a `-` prefix; hour segment omitted when zero.
String formatDuration(Duration duration) {
  final neg = duration.isNegative;
  duration = duration.abs();
  final h = duration.inHours;
  final m = duration.inMinutes.remainder(60);
  final s = duration.inSeconds.remainder(60);
  final mm = m.toString().padLeft(2, '0');
  final ss = s.toString().padLeft(2, '0');
  final body = h > 0 ? '$h:$mm:$ss' : '$mm:$ss';
  return neg ? '-$body' : body;
}
