import 'ffi/xhvp_library.dart';

/// 插件入口：加载原生库并初始化 GStreamer 运行时。
///
/// 在创建 [XueHuaPlayerController] 之前调用 [initialize] 一次：
///
/// ```dart
/// Future<void> main() async {
///   WidgetsFlutterBinding.ensureInitialized();
///   await XueHuaVideoPlayer.initialize();
///   runApp(const MyApp());
/// }
/// ```
class XueHuaVideoPlayer {
  const XueHuaVideoPlayer._();

  static bool _initialized = false;

  /// 加载原生库并初始化 C 播放器运行时。幂等；热重启后可重复调用。
  static Future<void> initialize() async {
    if (_initialized) return;
    final code = XhvpLibrary.instance.init();
    if (code != 0) {
      throw StateError('xhvp_init failed with code $code');
    }
    _initialized = true;
  }
}
