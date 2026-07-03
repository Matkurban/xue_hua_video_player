import 'rust/frb_generated.dart';

/// Entry point for the xue_hua_video_player plugin.
///
/// Call [initialize] once before creating a [XueHuaPlayerController]:
///
/// ```dart
/// Future<void> main() async {
///   WidgetsFlutterBinding.ensureInitialized();
///   await XueHuaVideoPlayer.initialize();
///   runApp(const MyApp());
/// }
/// ```
///
/// This replaces calling `RustLib.init()` directly and is safe to call more
/// than once, including after a Flutter hot restart.
class XueHuaVideoPlayer {
  const XueHuaVideoPlayer._();

  /// Loads the native library and initializes the Rust bridge.
  ///
  /// Idempotent within a session; on hot restart the bridge state is recreated,
  /// so a redundant "initialize twice" error is swallowed rather than surfaced.
  static Future<void> initialize() async {
    if (RustLib.instance.initialized) return;
    try {
      await RustLib.init();
    } on StateError {
      // Already initialized (e.g. after a hot restart) - safe to ignore.
    }
  }
}
