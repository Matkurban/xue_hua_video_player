import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/ffi/xhvp_library.dart';

void main() {
  test('xhvp_version from host dylib', () {
    final path =
        '${Directory.current.path}/native/build/host/libxue_hua_video_player.dylib';
    if (!File(path).existsSync()) {
      // Also accept .so on Linux CI.
      final so =
          '${Directory.current.path}/native/build/host/libxue_hua_video_player.so';
      if (!File(so).existsSync()) {
        // Skip when native host lib was not built in this environment.
        return;
      }
      XhvpLibrary.openPath(so);
    } else {
      XhvpLibrary.openPath(path);
    }
    expect(XhvpLibrary.instance.version(), isNotEmpty);
    expect(XhvpLibrary.instance.init(), 0);
  });
}
