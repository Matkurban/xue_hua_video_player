import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

import 'xhvp_bindings.dart';

/// Loads the native player library and exposes [XhvpBindings].
class XhvpLibrary {
  XhvpLibrary._(this.bindings);

  final XhvpBindings bindings;

  static XhvpLibrary? _instance;

  /// Process-wide singleton.
  static XhvpLibrary get instance => _instance ??= XhvpLibrary._(_open());

  static bool get isInitialized => _instance != null;

  static XhvpBindings _open() {
    final DynamicLibrary dylib;
    if (Platform.isMacOS || Platform.isIOS) {
      dylib = DynamicLibrary.process();
    } else if (Platform.isAndroid) {
      dylib = DynamicLibrary.open('libxue_hua_video_player.so');
    } else if (Platform.isWindows) {
      dylib = DynamicLibrary.open('xue_hua_video_player_plugin.dll');
    } else if (Platform.isLinux) {
      dylib = DynamicLibrary.open('libxue_hua_video_player_plugin.so');
    } else {
      throw UnsupportedError('Unsupported platform for XhvpLibrary');
    }
    return XhvpBindings(dylib);
  }

  /// For host unit tests that load a built dylib by path.
  static XhvpLibrary openPath(String path) {
    final lib = XhvpLibrary._(XhvpBindings(DynamicLibrary.open(path)));
    _instance = lib;
    return lib;
  }

  String version() {
    final ptr = bindings.xhvp_version();
    return ptr.cast<Utf8>().toDartString();
  }

  int init() => bindings.xhvp_init();

  void shutdown() => bindings.xhvp_shutdown();
}
