fn main() {
    // The GStreamer plugins we register statically on iOS (see
    // `register_ios_static_plugins` in `src/player.rs`) reference a handful of
    // Apple system frameworks and system libraries. The final Flutter app link
    // (see `ios/xue_hua_video_player.podspec`) links these for the app binary,
    // but `cargo build` also links the crate's `cdylib` artifact, and that link
    // must resolve the same symbols. Emit the flags for the iOS target only;
    // other platforms get their native deps elsewhere.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        for framework in [
            "AudioToolbox", // osxaudio: AudioQueue / AudioConverter / AudioUnit
            "AVFoundation",
            "AVFAudio", // osxaudio: AVAudioSessionCategory* constants
            "CoreMedia",
            "CoreVideo",
            "CoreAudio",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        for lib in ["resolv", "iconv"] {
            // e.g. res_9_ninit / res_9_nquery referenced by GStreamer core.
            println!("cargo:rustc-link-lib={lib}");
        }
    }
}
