import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/ffi/xhvp_library.dart';
import 'package:xue_hua_video_player/src/player/ffi_native_worker.dart';
import 'package:xue_hua_video_player/xue_hua_video_player.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('initialize warms FfiNativeWorker (host dylib)', () async {
    final dylib = _hostDylibPath();
    if (dylib == null) {
      // Skip when native host lib was not built in this environment.
      return;
    }
    XhvpLibrary.openPath(dylib);
    await XueHuaVideoPlayer.initialize();
    expect(XueHuaVideoPlayer.isInitialized, isTrue);
    expect(FfiNativeWorker.isStarted, isTrue);
  });
}

String? _hostDylibPath() {
  final dylib =
      '${Directory.current.path}/native/build/host/libxue_hua_video_player.dylib';
  if (File(dylib).existsSync()) {
    return dylib;
  }
  final so =
      '${Directory.current.path}/native/build/host/libxue_hua_video_player.so';
  if (File(so).existsSync()) {
    return so;
  }
  return null;
}
