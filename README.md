# xue_hua_video_player

A cross-platform Flutter **video player** plugin that decodes local and network
video with **GStreamer** (through a Rust [`flutter_rust_bridge`] core) and renders
frames into a Flutter **external texture** via [`irondash_texture`] (zero-copy on
the GPU where possible).

Supported platforms: **macOS, Windows, Linux, Android, iOS**.

> Scope: video playback only — open / play / pause / stop / seek / volume /
> mute / speed / looping, plus state / position / duration / resolution /
> buffering / EOS / error reporting. No recording, streaming, subtitle-track
> selection, etc.

## Usage

```dart
import 'package:xue_hua_video_player/xue_hua_video_player.dart';

// once at app start
await XueHuaVideoPlayer.initialize();

final controller = XueHuaPlayerController();
await controller.initialize();
await controller.open(
  VideoSource.network('https://example.com/video.mp4'),
  // or VideoSource.file('/path/to/video.mp4')
  // or VideoSource.asset('assets/sample.mp4')
  autoPlay: true,
);

// in your widget tree
XueHuaVideoView(controller: controller);

// controls
await controller.play();
await controller.pause();
await controller.seek(const Duration(seconds: 30));
await controller.setVolume(0.5);
await controller.setSpeed(1.5);
await controller.setLooping(true);

// when done
await controller.dispose();
```

`XueHuaPlayerController` exposes its state as fine-grained [`signals`]; read
`.value` inside a `SignalBuilder`/`Watch` so only dependent widgets rebuild. It
exposes `state`, `position`, `duration`, `videoSize`, `aspectRatio`,
`bufferingPercent`, `volume`, `speed`, `looping`, `muted`, `isPlaying`,
`isCompleted`, and `error` (all `ReadonlySignal`s). `XueHuaVideoView` also draws
an adaptive control bar by default (`showControls`, `controlsStyle`).

## Architecture

```
Dart:  XueHuaPlayerController ──FRB calls──► Rust API (api/player.rs)
       XueHuaVideoView (Texture) ◄──frames── irondash texture
Rust:  GstPlayer  playbin3 ─► videoconvert ─► appsink (RGBA) ─► FrameBuffer
                     │ bus messages ─► StreamSink<PlayerEvent> ─► Dart
```

- Decoding: `playbin3` with the video sink set to an `appsink` (wrapped in a
  `videoconvert` bin) forced to `video/x-raw,format=RGBA`, `max-buffers=1`,
  `drop=true`.
- Rendering: the appsink callback copies each frame into a shared buffer and
  calls `mark_frame_available`; irondash requests the frame on the raster thread.
- The texture is created on the platform main thread via the irondash run loop,
  and its id is handed to a Flutter `Texture` widget.

## Regenerating bindings

After changing the Rust API:

```bash
flutter_rust_bridge_codegen generate
```

## Native GStreamer setup (per platform)

The Rust core links GStreamer through `pkg-config` at build time, so a GStreamer
**development** install must be discoverable while building, and the runtime
libraries must be available (bundled or system-installed) when running.

### macOS

Install via Homebrew (used by `macos/xue_hua_video_player.podspec` by default):

```bash
brew install pkg-config gstreamer gst-plugins-base gst-plugins-good \
  gst-plugins-bad gst-libav
```

To use the official `GStreamer.framework` instead, set
`GSTREAMER_PKG_CONFIG_PATH` to its `.../Versions/1.0/lib/pkgconfig`.

For distribution you must bundle the GStreamer dylibs into the `.app` and fix
their load paths; the Homebrew setup above is intended for local development
(the example disables the App Sandbox so it can load the Homebrew dylibs).

### Linux

```bash
sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav
```

Runtime uses the system GStreamer libraries.

### Windows

Install the **runtime** and **development** GStreamer MSVC packages from
<https://gstreamer.freedesktop.org/download/>. The installer sets
`GSTREAMER_1_0_ROOT_MSVC_X86_64`, which `windows/CMakeLists.txt` uses to locate
headers/libs and to bundle the runtime DLLs next to the app. A `pkg-config`
(e.g. `choco install pkgconfiglite`) must be on `PATH`. Plugin DLLs live under
`lib/gstreamer-1.0`; set `GST_PLUGIN_SYSTEM_PATH` to the bundled copy at startup
when packaging.

### iOS (physical device)

Targets a physical arm64 iPhone. The prebuilt SDK does not ship an arm64
**Simulator** slice, so the Apple-Silicon iOS Simulator is not supported.

1. Download and install the **GStreamer iOS SDK** (`devel`), matching your
   desktop GStreamer major/minor version:
   ```bash
   curl -fLO https://gstreamer.freedesktop.org/data/pkg/ios/1.28.4/gstreamer-1.0-devel-1.28.4-ios-universal.pkg
   # user-domain install (no sudo); double-clicking the .pkg also works
   installer -pkg gstreamer-1.0-devel-1.28.4-ios-universal.pkg -target CurrentUserHomeDirectory
   ```
   This installs `GStreamer.framework` under
   `~/Library/Developer/GStreamer/iPhone.sdk` (override with `GSTREAMER_ROOT_IOS`).
2. `ios/xue_hua_video_player.podspec` already exports `PKG_CONFIG_PATH` +
   `PKG_CONFIG_ALLOW_CROSS=1` for the Rust cross-build and links the umbrella
   `-framework GStreamer`.
3. Build/run on a connected device: `flutter run -d <device>` (or
   `flutter build ios` / `--no-codesign` to verify the build).

### Android (arm64)

Targets arm64-v8a devices. Two things must be in place: the Gradle build (see
"cargokit + Gradle 9" below) and the GStreamer Android SDK.

1. Download and extract the **GStreamer Android SDK** (prebuilt universal):
   ```bash
   curl -fLO https://gstreamer.freedesktop.org/data/pkg/android/1.28.4/gstreamer-1.0-android-universal-1.28.4.tar.xz
   mkdir -p ~/Library/Developer/GStreamer/android/1.28.4
   tar -xf gstreamer-1.0-android-universal-1.28.4.tar.xz \
     -C ~/Library/Developer/GStreamer/android/1.28.4 --strip-components=1
   ```
   Result: per-ABI dirs (`arm64/`, `armv7/`, `x86/`, `x86_64/`), each with
   `lib/pkgconfig`.
2. The Rust cross-build finds GStreamer via [`rust/.cargo/config.toml`](rust/.cargo/config.toml),
   which sets `PKG_CONFIG_ALLOW_CROSS=1` and
   `PKG_CONFIG_PATH_aarch64-linux-android` to the SDK's `arm64/lib/pkgconfig`.
   Update those absolute paths if you installed the SDK elsewhere.
3. The GStreamer shared libraries + the plugins the player needs are bundled in
   the example under
   `example/android/app/src/main/jniLibs/arm64-v8a/` and extracted at runtime
   (`useLegacyPackaging = true`). Rust points GStreamer's plugin/GIO scanner at
   the app's native library directory before `gst::init()`
   (`setup_android_gst_env` in `rust/src/player.rs`), so no `GStreamer.init`
   Java class is required.
4. Only arm64-v8a is wired up (`abiFilters` in the example app and the ABI trim
   in `cargokit/gradle/plugin.gradle`). To support the emulator, add the x86/x64
   `PKG_CONFIG_PATH_*` entries in `rust/.cargo/config.toml`, re-enable those ABIs
   in cargokit, and bundle the matching `.so`.

#### cargokit + Gradle 9

cargokit (bundled by this plugin and by `irondash_engine_context`) calls
`project.exec {}`, which Gradle 9 removed. Both copies are patched to use an
injected `ExecOperations` instead:

- `cargokit/gradle/plugin.gradle` (in this repo)
- `~/.pub-cache/hosted/pub.dev/irondash_engine_context-<version>/cargokit/gradle/plugin.gradle`

The pub-cache copy is reset by `flutter pub get` / cache cleaning, so re-apply
that one-line change (`project.exec` -> `getExecOperations().exec`, plus the
`@Inject abstract ExecOperations getExecOperations()` getter and its imports) if
Android builds start failing again with "Could not find method exec()".

## Vendored dependency patch

`rust/vendor/irondash_texture` is a local copy of `irondash_texture` 0.5.0 with a
macOS/iOS fix: the upstream backing `IOSurface` uses `bytesPerRow = width * 4`,
which fails Metal's row-alignment requirement on the current Flutter renderer
(`Could not create Metal texture from pixel buffer: CVReturn -6684`). The
vendored copy aligns the stride to 256 bytes and uploads row-by-row. It is wired
in through `[patch.crates-io]` in `rust/Cargo.toml`.

[`flutter_rust_bridge`]: https://pub.dev/packages/flutter_rust_bridge
[`irondash_texture`]: https://crates.io/crates/irondash_texture
[`signals`]: https://pub.dev/packages/signals
