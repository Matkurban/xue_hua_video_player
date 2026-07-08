//! iOS/macOS 沙盒可写路径与 GStreamer 环境变量配置。
//!
//! Writable sandbox paths and GStreamer environment configuration for iOS/macOS.
//!
//! 在 `gst::init()` 之前设置 GLib/GStreamer 期望的 `HOME`、`TMPDIR`、`GST_REGISTRY`
//! 等变量，并配置平台特有的插件路径与解码器优先级。

/// 配置 iOS/macOS 沙盒内可写的临时目录与 GStreamer 注册表路径。
/// Configures writable temp directories and GStreamer registry path for iOS/macOS sandboxes.
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 平台 / Platform
/// - 仅 **iOS** 与 **macOS** / **iOS** and **macOS** only.
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub fn setup_sandbox_writable_env() {
    use std::path::PathBuf;

    let tmp = std::env::temp_dir();
    let tmp_str = tmp.to_string_lossy().to_string();
    let home: PathBuf = tmp
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| tmp.clone());
    let home_str = home.to_string_lossy().to_string();
    let registry = tmp.join("gstreamer-registry.bin");
    let registry_str = registry.to_string_lossy().to_string();

    std::env::set_var("ORC_CODE", "backup");
    std::env::set_var("HOME", &home_str);
    std::env::set_var("TMPDIR", &tmp_str);
    std::env::set_var("TMP", &tmp_str);
    std::env::set_var("TEMP", &tmp_str);
    std::env::set_var("XDG_CACHE_HOME", &tmp_str);
    std::env::set_var("XDG_DATA_HOME", &tmp_str);
    std::env::set_var("XDG_CONFIG_HOME", &tmp_str);
    std::env::set_var("XDG_RUNTIME_DIR", &tmp_str);
    std::env::set_var("XDG_DATA_DIRS", &tmp_str);
    std::env::set_var("XDG_CONFIG_DIRS", &tmp_str);
    std::env::set_var("GST_REGISTRY", &registry_str);
}

/// 在 `gst::init()` 之前准备 iOS 进程环境。
/// Prepares the iOS process environment before `gst::init()`.
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 平台 / Platform
/// - 仅 **iOS** / **iOS** only.
/// - 设置 VideoToolbox 硬件解码优先级，确保真机输出 IOSurface 支持的 `CVPixelBuffer`
///   （软件 `avdec_*` 在真机上会导致黑屏）。
///   Sets VideoToolbox hardware decode priority so decoded frames are IOSurface-backed
///   `CVPixelBuffers` (software `avdec_*` output renders black on real devices).
#[cfg(target_os = "ios")]
pub fn setup_ios_env() {
    setup_sandbox_writable_env();

    // Prefer VideoToolbox hardware decode so decoded frames are IOSurface-backed
    // CVPixelBuffers. `avsamplebufferlayersink`'s `AVSampleBufferDisplayLayer`
    // only renders IOSurface-backed buffers on real devices (software `avdec_*`
    // output is plain system memory -> black frame on device). Mirrors macOS.
    std::env::set_var(
        "GST_PLUGIN_FEATURE_RANK",
        "vtdec:PRIMARY,vtdec_hw:PRIMARY,vtdemux:PRIMARY,avdec_h264:SECONDARY,avdec_h265:SECONDARY",
    );
}

/// 定位应用包内或系统框架中的 GStreamer 库目录。
/// Locates GStreamer libraries inside the app bundle or the system framework.
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// - 找到时返回 `lib` 目录路径（如 `…/GStreamer.framework/Versions/1.0/lib`）；
///   未找到时返回 `None`。
///   Returns the `lib` directory path when found (e.g.
///   `…/GStreamer.framework/Versions/1.0/lib`); `None` otherwise.
///
/// # 平台 / Platform
/// - 仅 **macOS** / **macOS** only.
/// - 优先检查可执行文件旁的嵌入式框架，再回退到 `/Library/Frameworks/…`。
#[cfg(target_os = "macos")]
pub fn bundled_gstreamer_lib_dir() -> Option<std::path::PathBuf> {
    use std::path::{Path, PathBuf};

    if let Ok(exe) = std::env::current_exe() {
        if let Some(contents) = exe.parent().and_then(Path::parent) {
            let embedded = contents
                .join("Frameworks")
                .join("GStreamer.framework")
                .join("Versions")
                .join("1.0")
                .join("lib");
            if embedded.is_dir() {
                return Some(embedded);
            }
        }
    }

    let system = PathBuf::from("/Library/Frameworks/GStreamer.framework/Versions/1.0/lib");
    if system.is_dir() {
        return Some(system);
    }
    None
}

/// 在 `gst::init()` 之前准备 macOS 沙盒与嵌入式 GStreamer.framework 路径。
/// Prepares macOS sandbox and embedded GStreamer.framework paths before `gst::init()`.
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 平台 / Platform
/// - 仅 **macOS** / **macOS** only.
/// - 设置 `GST_PLUGIN_SYSTEM_PATH`、`GIO_MODULE_DIR` 与解码器优先级。
#[cfg(target_os = "macos")]
pub fn setup_macos_env() {
    setup_sandbox_writable_env();

    if let Some(lib_dir) = bundled_gstreamer_lib_dir() {
        let plugins = lib_dir.join("gstreamer-1.0");
        if plugins.is_dir() {
            std::env::set_var("GST_PLUGIN_SYSTEM_PATH", plugins.to_string_lossy().as_ref());
        }
        let gio_modules = lib_dir.join("gio").join("modules");
        if gio_modules.is_dir() {
            std::env::set_var("GIO_MODULE_DIR", gio_modules.to_string_lossy().as_ref());
        }
    }

    std::env::set_var(
        "GST_PLUGIN_FEATURE_RANK",
        "vtdec:PRIMARY,vtdemux:PRIMARY,avdec_h264:SECONDARY,avdec_h265:SECONDARY",
    );
}
