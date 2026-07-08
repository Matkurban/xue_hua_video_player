//! FRB 应用初始化钩子 / FRB application initialization hook.

/// FRB 启动时调用，配置默认用户工具（日志等）/ Called by FRB at startup to configure default user utilities (logging, etc.).
///
/// # 参数 / Parameters
/// - 无 / None
///
/// # 返回值 / Returns
/// - 无 / None
///
/// # 线程 / Threading
/// - 可在任意线程调用；由 Dart `RustLib.init()` 在应用启动时触发一次。
/// - May be called on any thread; invoked once at app startup via Dart `RustLib.init()`.
#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}
