/// Writable sandbox paths and registry location shared by iOS/macOS.
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

/// Prepares the process environment GStreamer/GLib expect, before `gst::init()`.
#[cfg(target_os = "ios")]
pub fn setup_ios_env() {
    setup_sandbox_writable_env();

    std::env::set_var(
        "GST_PLUGIN_FEATURE_RANK",
        "vtdec:PRIMARY,vtdemux:PRIMARY,avdec_h264:SECONDARY,avdec_h265:SECONDARY",
    );
}

/// Locates GStreamer libraries inside the app bundle, or the system framework.
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

/// Prepares macOS sandbox + embedded GStreamer.framework paths before `gst::init()`.
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
