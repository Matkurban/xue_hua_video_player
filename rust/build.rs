//! Cargo 构建脚本 / Cargo build script.
//!
//! 负责编译平台特定的 Objective-C shim 并在 iOS 目标上声明 GStreamer 静态插件
//! 所需的系统 framework 链接标志。
//!
//! Compiles platform-specific Objective-C shims and emits system framework link flags
//! required by GStreamer static plugins on iOS.

fn main() {
    // iOS 上静态注册的 GStreamer 插件（见 `gst/ios_plugins.rs`）引用若干 Apple
    // 系统 framework 与库。Flutter 应用最终链接（见 `ios/xue_hua_video_player.podspec`）
    // 也会链接这些符号，但 `cargo build` 构建 cdylib 时同样必须解析它们。
    //
    // Static GStreamer plugins registered on iOS (see `gst/ios_plugins.rs`) reference
    // Apple system frameworks. The Flutter app link resolves them too, but `cargo build`
    // of the cdylib artifact must also resolve the same symbols.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let shim = manifest_dir.join("../ios/Classes/XueHuaMainThreadShim.m");
        println!("cargo:rerun-if-changed={}", shim.display());
        cc::Build::new()
            .file(shim)
            .flag("-fobjc-arc")
            .compile("xhvp_ios_main_thread_shim");

        for framework in [
            "UIKit",
            "QuartzCore",
            "CoreGraphics",
            "IOSurface",
            "Metal",
            "Foundation",
            "AudioToolbox", // osxaudio: AudioQueue / AudioConverter / AudioUnit
            "AVFoundation",
            "AVFAudio", // osxaudio: AVAudioSessionCategory* constants
            "AssetsLibrary",
            "CoreMedia",
            "CoreVideo",
            "CoreAudio",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        for lib in ["resolv", "iconv", "c++"] {
            // 例如 GStreamer core 引用的 res_9_ninit / res_9_nquery。
            // e.g. res_9_ninit / res_9_nquery referenced by GStreamer core.
            println!("cargo:rustc-link-lib={lib}");
        }
    }
}
