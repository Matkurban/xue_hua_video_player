// swift-tools-version: 5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let gstRoot = Context.environment["GSTREAMER_ROOT_IOS"]
    ?? "\(Context.environment["HOME"] ?? "")/Library/Developer/GStreamer/iPhone.sdk"

let package = Package(
    name: "xue_hua_video_player",
    platforms: [
        .iOS("13.0"),
    ],
    products: [
        .library(name: "xue-hua-video-player", targets: ["xue_hua_video_player"]),
    ],
    dependencies: [
        .package(name: "FlutterFramework", path: "../FlutterFramework"),
    ],
    targets: [
        .target(
            name: "xue_hua_video_player",
            dependencies: [
                .product(name: "FlutterFramework", package: "FlutterFramework"),
                "xhvp_player_c",
            ]
        ),
        .target(
            name: "xhvp_player_c",
            path: "NativeCore",
            exclude: [
                "src/android_jni.c",
            ],
            publicHeadersPath: "include",
            cSettings: [
                .headerSearchPath("include"),
                .headerSearchPath("src"),
                .define("XHVP_BUILDING"),
                .define("TARGET_OS_IPHONE", to: "1"),
                .unsafeFlags([
                    "-I\(gstRoot)/GStreamer.framework/Headers",
                ]),
            ],
            linkerSettings: [
                .linkedFramework("GStreamer"),
                .linkedFramework("UIKit"),
                .linkedFramework("QuartzCore"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("IOSurface"),
                .linkedFramework("Metal"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("CoreMedia"),
                .linkedFramework("CoreVideo"),
                .linkedFramework("CoreAudio"),
                .linkedFramework("AVFoundation"),
                .linkedFramework("AVFAudio"),
                .linkedFramework("AssetsLibrary"),
                .linkedFramework("AudioToolbox"),
                .linkedFramework("VideoToolbox"),
                .linkedFramework("OpenGLES"),
                .linkedFramework("Foundation"),
                .linkedFramework("Security"),
                .linkedLibrary("iconv"),
                .linkedLibrary("resolv"),
                .linkedLibrary("z"),
                .linkedLibrary("bz2"),
                .unsafeFlags([
                    "-F\(gstRoot)",
                ]),
            ]
        ),
    ]
)
