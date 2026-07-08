import 'rust/frb_generated.dart';

/// 插件入口：加载原生库并初始化 Rust 桥 / Plugin entry: loads the native library and initializes the Rust bridge.
///
/// 在创建 [XueHuaPlayerController] 之前调用 [initialize] 一次：
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
/// 替代直接调用 `RustLib.init()`；同一会话内可重复调用，热重启后亦安全。
/// Replaces calling `RustLib.init()` directly; safe to call more than once per session, including after hot restart.
class XueHuaVideoPlayer {
  const XueHuaVideoPlayer._();

  /// 加载原生库并初始化 flutter_rust_bridge / Loads the native library and initializes flutter_rust_bridge.
  ///
  /// 会话内幂等；热重启会重建桥接状态，重复的「已初始化」错误会被吞掉而非抛出。
  /// Idempotent within a session; on hot restart the bridge is recreated and redundant init errors are swallowed.
  static Future<void> initialize() async {
    if (RustLib.instance.initialized) return;
    try {
      await RustLib.init();
    } on StateError {
      // Already initialized (e.g. after a hot restart) - safe to ignore.
    }
  }
}
