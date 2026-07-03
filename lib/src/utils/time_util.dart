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
