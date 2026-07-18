import 'package:flutter/foundation.dart';

/// Debug-only init phase timing (`[xhvp-init-timing] phase=Nms`).
void xhvpInitTiming(String phase, Stopwatch sw) {
  if (kDebugMode) {
    debugPrint('[xhvp-init-timing] $phase=${sw.elapsedMilliseconds}ms');
  }
}

/// Times [action] and logs [phase] in debug mode.
T xhvpTimed<T>(String phase, T Function() action) {
  final sw = Stopwatch()..start();
  final result = action();
  xhvpInitTiming(phase, sw);
  return result;
}

/// Times async [action] and logs [phase] in debug mode.
Future<T> xhvpTimedAsync<T>(String phase, Future<T> Function() action) async {
  final sw = Stopwatch()..start();
  final result = await action();
  xhvpInitTiming(phase, sw);
  return result;
}
