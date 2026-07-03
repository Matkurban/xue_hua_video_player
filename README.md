# xue_hua_video_player

English | [简体中文](README.zh-CN.md)

A cross-platform Flutter **video player** plugin that decodes local and network
video with **GStreamer** (through a Rust [`flutter_rust_bridge`] core) and renders
frames into a Flutter **external texture** via [`irondash_texture`] (zero-copy on
the GPU where possible).

- Repository: <https://github.com/Matkurban/xue_hua_video_player>
- Author: Matkurban &lt;3496354336@qq.com&gt;

Supported platforms: **Android, iOS, macOS, Windows, Linux**.

> Scope: video playback only — open / play / pause / stop / seek / volume /
> mute / speed / looping, plus state / position / duration / resolution /
> buffering / EOS / error reporting. It does **not** do recording, streaming
> (as a server), or subtitle-track selection.

## Table of contents

- [Features](#features)
- [Platform support](#platform-support)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Integrating into your app (read this first)](#integrating-into-your-app-read-this-first)
- [Permissions & platform configuration](#permissions--platform-configuration)
- [API reference](#api-reference)
- [Native GStreamer setup (per platform)](#native-gstreamer-setup-per-platform)
- [Precompiled binaries](#precompiled-binaries)
- [Architecture](#architecture)
- [Troubleshooting](#troubleshooting)
- [Maintainers](#maintainers)
- [License](#license)

## Features

- Local files, Flutter assets, and network URLs (`http(s)://`, `rtsp://`, ...).
- Play / pause / stop / seek / looping.
- Volume, mute, and playback speed control.
- Reactive state via fine-grained [`signals`]: state, position, duration, video
  size, aspect ratio, buffering %, volume, speed, looping, muted, and errors.
- A drop-in `XueHuaVideoView` widget with a built-in, auto-hiding, themeable
  control bar (Material / Cupertino / adaptive).
- GPU texture rendering (no per-frame copy to the Dart side).

## Platform support

| Platform | Min version | Architectures | GStreamer runtime |
| --- | --- | --- | --- |
| Android | API 24 (7.0) | `arm64-v8a`, `armeabi-v7a`, `x86`, `x86_64` | Bundled in the plugin (no setup) |
| iOS | 13.0 | Physical `arm64` device (no Simulator) | GStreamer iOS SDK (static framework) |
| macOS | 10.13 | x86_64 / arm64 | Homebrew or `GStreamer.framework` |
| Windows | 10+ | x86_64 | GStreamer MSVC runtime |
| Linux | — | x86_64 | System GStreamer + GTK 3 |

> The Apple-Silicon iOS **Simulator** is not supported because the prebuilt iOS
> SDK does not ship an arm64 simulator slice.

## Installation

This package is distributed via Git. Add it to your app's `pubspec.yaml`:

```yaml
dependencies:
  xue_hua_video_player:
    git:
      url: https://github.com/Matkurban/xue_hua_video_player.git
      ref: v1.0.0
```

Then:

```bash
flutter pub get
```

The Rust core is fetched as a **precompiled binary** by default, so your machine
does **not** need the Rust toolchain (see [Precompiled binaries](#precompiled-binaries)).
Each desktop/iOS platform still needs the GStreamer SDK available at build/run
time; Android bundles everything (see the sections below).

## Quick start

```dart
import 'package:flutter/material.dart';
import 'package:xue_hua_video_player/xue_hua_video_player.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // Load the native library once at app start.
  await XueHuaVideoPlayer.initialize();
  runApp(const MyApp());
}

class PlayerPage extends StatefulWidget {
  const PlayerPage({super.key});
  @override
  State<PlayerPage> createState() => _PlayerPageState();
}

class _PlayerPageState extends State<PlayerPage> {
  final controller = XueHuaPlayerController();

  @override
  void initState() {
    super.initState();
    controller.initialize().then((_) {
      controller.open(
        VideoSource.network('https://example.com/video.mp4'),
        autoPlay: true,
      );
    });
  }

  @override
  void dispose() {
    controller.dispose(); // always dispose to release the native player
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: XueHuaVideoView(controller: controller),
    );
  }
}
```

Sources can also be a local file or a bundled asset:

```dart
await controller.open(const VideoSource.file('/path/to/video.mp4'));
await controller.open(const VideoSource.asset('assets/sample.mp4'));
```

Assets are copied to a temporary file on first use because GStreamer can only
read filesystem paths and URLs, not the Flutter asset bundle.

## Integrating into your app (read this first)

1. **Call `XueHuaVideoPlayer.initialize()` once** before creating any controller
   (typically in `main()` after `WidgetsFlutterBinding.ensureInitialized()`). It
   is idempotent and safe to call again after a hot restart.
2. **Create and `initialize()` a `XueHuaPlayerController` per video surface.** The
   controller owns a native player plus a GPU texture; both are created during
   `initialize()`.
3. **Always `dispose()` the controller** when the surface goes away — this stops
   the pipeline, cancels the event stream, and releases the texture on the
   platform thread. Leaking a controller leaks a native pipeline.
4. **Read state inside `SignalBuilder`/`Watch`.** Every state field is a
   `ReadonlySignal`; reading `.value` outside a reactive builder will not rebuild
   your widget when it changes.
5. **Android ships all four ABIs** (`arm64-v8a`, `armeabi-v7a`, `x86`,
   `x86_64`). You may optionally narrow this with `abiFilters` to shrink your APK
   (see below).
6. **iOS/macOS/Windows/Linux require the GStreamer SDK** at build time (and its
   runtime libraries at run time). See
   [Native GStreamer setup](#native-gstreamer-setup-per-platform).

## Permissions & platform configuration

Playback of **network** video requires per-platform configuration. Local files
and assets work with no extra permissions (aside from normal file access).

### Android

Add the internet permission to your app's
`android/app/src/main/AndroidManifest.xml` (outside `<application>`):

```xml
<uses-permission android:name="android.permission.INTERNET"/>
```

To allow plaintext `http://` (Android blocks cleartext by default on API 28+),
set `usesCleartextTraffic` on `<application>`:

```xml
<application
    android:usesCleartextTraffic="true"
    ...>
```

The plugin ships all four ABIs (`arm64-v8a`, `armeabi-v7a`, `x86`, `x86_64`), so
no `abiFilters` are required. If you want to shrink your APK to only the ABIs you
ship, narrow the set in `android/app/build.gradle(.kts)`:

```kotlin
android {
    defaultConfig {
        ndk {
            // Optional: keep only the ABIs you need (arm64-v8a covers most
            // modern phones; x86_64 is useful for emulators).
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }
}
```

Prefer per-ABI APKs or an Android App Bundle so each device only downloads its
own ABI — the bundled GStreamer runtime is large (~13–18 MB per ABI).

> The plugin ships `x86` (32-bit) libraries for completeness, but current Flutter
> no longer builds 32-bit x86 apps, so in practice `arm64-v8a`, `armeabi-v7a`,
> and `x86_64` are what a Flutter app will actually package.

Notes:

- The plugin's `AndroidManifest.xml` already sets
  `android:extractNativeLibs="true"`; it merges into your app so the bundled
  `libgstreamer_android.so` is extracted to disk for the dynamic loader.
- Some transitive plugins require a recent `compileSdk`. If AAR metadata checks
  fail, force `compileSdk = 36` across subprojects (the example does this in its
  `android/build.gradle.kts`).

### iOS

- **Minimum deployment target: iOS 13.0.** Physical `arm64` device only (no
  Simulator).
- GStreamer performs networking with its own sockets + OpenSSL, **not** through
  `NSURLSession`, so App Transport Security (ATS) does **not** block playback and
  no `NSAppTransportSecurity` entry is required for GStreamer streams (both
  `http://` and `https://` work).
- The static iOS framework's plugins and TLS backend are registered in the Rust
  core (`rust/src/player.rs`, `#[cfg(target_os = "ios")]`). HTTPS certificate
  verification is intentionally relaxed (`ssl-strict = false`) so streams from
  hosts without a bundled CA chain still play — see the
  [security note](#security-note-on-https).

### macOS

Network video requires the **outgoing network** entitlement. Add it to both
`macos/Runner/DebugProfile.entitlements` and `Release.entitlements`:

```xml
<key>com.apple.security.network.client</key>
<true/>
```

If you keep the **App Sandbox enabled**, you must bundle the GStreamer dylibs
inside the `.app` and add:

```xml
<key>com.apple.security.cs.disable-library-validation</key>
<true/>
```

The bundled example **disables** the App Sandbox so it can load the Homebrew
dylibs directly during development; production apps should bundle GStreamer and
keep the sandbox on.

### Windows

- Install the GStreamer **MSVC** package (development files + runtime) from
  <https://gstreamer.freedesktop.org/download/>.
- Ensure the runtime DLLs are discoverable at run time — add
  `...\1.0\msvc_x86_64\bin` to `PATH` or bundle the DLLs next to your `.exe`.
- Plugin DLLs live under `lib\gstreamer-1.0`; when packaging, point
  `GST_PLUGIN_SYSTEM_PATH` at the bundled copy at startup.

### Linux

- No app permission is required. Install the system GStreamer development and
  plugin packages plus GTK 3 (see [Native setup](#linux-1)).

### Security note on HTTPS

To maximize compatibility with hosts whose certificate chains are not present in
the (minimal) bundled trust store, the pipeline sets `ssl-strict = false` on the
HTTP source, which **skips server-certificate verification**. This removes
protection against man-in-the-middle attacks. If you need strict verification,
bundle a CA certificate database and configure GLib's default `GTlsDatabase`
(open an issue if you want this made configurable).

## API reference

### `XueHuaVideoPlayer.initialize()`

Static, one-time initialization of the native library / Rust bridge. Call once
before using any controller.

### `XueHuaPlayerController`

| Method | Description |
| --- | --- |
| `initialize()` | Creates the native player + texture and subscribes to events. |
| `open(VideoSource, {bool autoPlay})` | Loads a source; optionally starts playback. |
| `play()` / `pause()` / `stop()` | Playback transport. |
| `togglePlayPause()` | Play if paused, pause if playing. |
| `seek(Duration)` | Seek to a position. |
| `setVolume(double)` | Volume in `0.0..1.0`. |
| `setMuted(bool)` / `toggleMuted()` | Mute control. |
| `setSpeed(double)` | Playback speed multiplier. |
| `setLooping(bool)` | Loop at end-of-stream. |
| `queryPosition()` / `queryDuration()` | Query the pipeline directly. |
| `dispose()` | Tear down the player and release all resources. |

Reactive state (all `ReadonlySignal`s; read `.value` in a `SignalBuilder`):
`state`, `position`, `duration`, `videoSize`, `aspectRatio`, `bufferingPercent`,
`volume`, `speed`, `looping`, `muted`, `isPlaying`, `isCompleted`, `error`,
`textureId`, `initialized`.

`PlayerState`: `idle`, `ready`, `buffering`, `playing`, `paused`, `stopped`,
`completed`, `error`.

### `VideoSource`

- `VideoSource.network(String url)`
- `VideoSource.file(String path)` (accepts a plain path or a `file://` URI)
- `VideoSource.asset(String assetKey)`

### `XueHuaVideoView`

A `StatelessWidget` that renders the controller's texture and, by default, an
adaptive control bar.

| Parameter | Default | Description |
| --- | --- | --- |
| `controller` | required | The `XueHuaPlayerController` to render. |
| `fit` | `BoxFit.contain` | How the video is inscribed. |
| `backgroundColor` | black | Letterbox / background color. |
| `showControls` | `true` | Overlay the built-in control bar. |
| `controlsStyle` | `adaptive` | `adaptive` / `material` / `cupertino`. |

### Theming the controls

Register a `VideoControlsTheme` in `ThemeData.extensions` to customize the
control bar, or rely on the built-in `VideoControlsTheme.material()` /
`VideoControlsTheme.cupertino()` presets:

```dart
MaterialApp(
  theme: ThemeData(
    extensions: const [/* your */ VideoControlsTheme.material()],
  ),
);
```

## Native GStreamer setup (per platform)

The Rust core links GStreamer at build time, so a GStreamer **development**
install must be discoverable while building, and the runtime libraries must be
available (bundled or system-installed) when running.

### macOS

Install via Homebrew (used by `macos/xue_hua_video_player.podspec` by default):

```bash
brew install pkg-config gstreamer gst-plugins-base gst-plugins-good \
  gst-plugins-bad gst-libav
```

To use the official `GStreamer.framework` instead, set
`GSTREAMER_PKG_CONFIG_PATH` to its `.../Versions/1.0/lib/pkgconfig`.

For distribution you must bundle the GStreamer dylibs into the `.app` and fix
their load paths; the Homebrew setup above is intended for local development.

### Linux

```bash
sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav \
  libgtk-3-dev
```

Runtime uses the system GStreamer libraries. `libgtk-3-dev` is required because
the texture backend links GTK 3.

### Windows

> GStreamer 1.28+ ships a single **Inno Setup `.exe`** installer (the older
> `.msi` packages, including the separate `-devel` package, were removed). One
> installer bundles both runtime and development files.

1. Download `gstreamer-1.0-msvc-x86_64-<version>.exe` (the MSVC build) from
   <https://gstreamer.freedesktop.org/download/> and run it. Choose the
   **"Runtime and development headers"** setup type so the headers/`.lib`/`.pc`
   files are installed.
2. The GUI installer sets `GSTREAMER_1_0_ROOT_MSVC_X86_64`, which
   `windows/CMakeLists.txt` uses to locate headers/libs and to bundle the runtime
   DLLs next to the app. (In headless/silent installs this variable may not be
   set reliably — set it manually if needed.)
3. A `pkg-config` must be on `PATH` (e.g. `choco install pkgconfiglite`).
4. Plugin DLLs live under `lib/gstreamer-1.0`; set `GST_PLUGIN_SYSTEM_PATH` to
   the bundled copy at startup when packaging.

### iOS (physical device)

Targets a physical arm64 iPhone. The prebuilt SDK does not ship an arm64
Simulator slice, so the Apple-Silicon iOS Simulator is not supported.

1. Download and install the **GStreamer iOS SDK** (`devel`), matching your
   desktop GStreamer major/minor version:

   ```bash
   curl -fLO https://gstreamer.freedesktop.org/data/pkg/ios/1.28.4/gstreamer-1.0-devel-1.28.4-ios-universal.pkg
   # user-domain install (no sudo); double-clicking the .pkg also works
   installer -pkg gstreamer-1.0-devel-1.28.4-ios-universal.pkg -target CurrentUserHomeDirectory
   ```

   This installs `GStreamer.framework` under
   `~/Library/Developer/GStreamer/iPhone.sdk` (override with `GSTREAMER_ROOT_IOS`).
2. `ios/xue_hua_video_player.podspec` already exports the `system-deps` overrides
   and `PKG_CONFIG_ALLOW_CROSS=1` for the Rust cross-build and links the umbrella
   `-framework GStreamer`.
3. Build/run on a connected device: `flutter run -d <device>` (or
   `flutter build ios --no-codesign` to verify the build).

Because the iOS `GStreamer.framework` is static, its plugins are not
auto-discovered. The Rust core registers the ones needed for playback and the
OpenSSL TLS backend (`register_ios_static_plugins()` /
`register_ios_tls_backend()` in `rust/src/player.rs`, guarded by
`#[cfg(target_os = "ios")]`), and prepares the runtime environment
(`ORC_CODE=backup`, `HOME`/`TMPDIR`/`XDG_*`) before `gst::init()`. Add to the
plugin list if you need an element that isn't registered yet.

### Android (all ABIs)

Supports `arm64-v8a`, `armeabi-v7a`, `x86`, and `x86_64`. **Consumers need no
GStreamer setup**: the plugin bundles the entire GStreamer runtime.
`android/src/main/jniLibs/<abi>/` ships the umbrella `libgstreamer_android.so`
(all of GStreamer + its plugins, linked statically) and `libc++_shared.so` for
each ABI; these are packaged into the plugin AAR and merged into the app. The
Rust `libxue_hua_video_player.so` is fetched as a precompiled binary per ABI. The
Rust core registers the static plugins itself (`gst_init_static_plugins()`), so
no `GStreamer.init` Java class is required.

A consuming app builds for all four ABIs by default. To reduce APK size, narrow
`abiFilters` (see [Permissions & platform configuration](#android)) or ship an
App Bundle. See the "Regenerating the bundled `.so`" section below if you are a
maintainer who needs to rebuild the umbrella libraries.

#### Regenerating the bundled `.so` (maintainers)

The GStreamer Android SDK is only needed by maintainers who rebuild the Rust
library from source (instead of using the precompiled binary) or regenerate the
umbrella `.so`.

**1. Download + extract the GStreamer Android SDK** (per-ABI top-level dirs):

```bash
curl -fLO https://gstreamer.freedesktop.org/data/pkg/android/1.28.4/gstreamer-1.0-android-universal-1.28.4.tar.xz
mkdir -p ~/Library/Developer/GStreamer/android/1.28.4
# NOTE: no --strip-components; the tarball's top level is arm64/ armv7/ x86/ x86_64/
tar -xf gstreamer-1.0-android-universal-1.28.4.tar.xz \
  -C ~/Library/Developer/GStreamer/android/1.28.4
```

**2. Build `libgstreamer_android.so`** for all ABIs from the recipe in
`android/gstreamer_build` (`jni/Application.mk` sets
`APP_ABI := arm64-v8a armeabi-v7a x86 x86_64`; edit the `GSTREAMER_PLUGINS` list
to add codecs):

```bash
cd android/gstreamer_build
export GSTREAMER_ROOT_ANDROID="$HOME/Library/Developer/GStreamer/android/1.28.4"
~/Library/Android/sdk/ndk/<ndk-version>/ndk-build \
  NDK_PROJECT_PATH=. NDK_APPLICATION_MK=jni/Application.mk -j4
# -> libs/<abi>/libgstreamer_android.so (+ libc++_shared.so) for each ABI
```

**3. Install the umbrella `.so`** into `jniLibs` (committed, bundled at runtime)
and into the SDK's per-ABI `lib` dir (for the Rust link step). The GStreamer SDK
uses `armv7`/`arm64` folder names, while jniLibs use `armeabi-v7a`/`arm64-v8a`:

```bash
GST=~/Library/Developer/GStreamer/android/1.28.4
declare -A SDK=( [arm64-v8a]=arm64 [armeabi-v7a]=armv7 [x86]=x86 [x86_64]=x86_64 )
for abi in arm64-v8a armeabi-v7a x86 x86_64; do
  cp libs/$abi/libgstreamer_android.so "$GST/${SDK[$abi]}/lib/"          # Rust link
  mkdir -p ../src/main/jniLibs/$abi
  cp libs/$abi/libgstreamer_android.so libs/$abi/libc++_shared.so \
    ../src/main/jniLibs/$abi/                                            # runtime
done
```

## Precompiled binaries

The Rust core is built with [cargokit], which supports **precompiled binaries**
so that consumers of this plugin do **not** need the Rust toolchain. When a
consumer builds their app, cargokit computes a `crate-hash` from the Rust sources
(`rust/src/**`, `Cargo.toml`, `Cargo.lock`, `cargokit.yaml`) and downloads a
signed, prebuilt library for that hash from this repo's GitHub Releases instead
of running `cargo`.

> Precompiled binaries only remove the **Rust toolchain** requirement. The
> artifact still references GStreamer symbols, so iOS/macOS/Linux/Windows
> consumers still need the GStreamer SDK at link/runtime. Android is the
> exception: the plugin bundles the GStreamer runtime, so it needs neither Rust
> nor the GStreamer SDK.

Behavior:

- Consumers **without** Rust installed automatically use the precompiled binary.
- Consumers **with** Rust installed build from source by default. To force the
  precompiled path, add a `cargokit_options.yaml` next to the app's
  `pubspec.yaml` with:

  ```yaml
  use_precompiled_binaries: true
  ```

Configuration lives in [`rust/cargokit.yaml`](rust/cargokit.yaml) (the download
URL prefix and the ed25519 **public** key used to verify signatures).

### Publishing (maintainers)

Precompiled binaries are produced by
[`.github/workflows/precompile_binaries.yml`](.github/workflows/precompile_binaries.yml)
on every push to `main` (macOS x86_64/arm64, iOS arm64, Linux x86_64, Windows
x86_64, Android arm64-v8a/armeabi-v7a/x86/x86_64) and uploaded to a release
tagged `precompiled_<hash>`.

One-time setup in the GitHub repo (`Matkurban/xue_hua_video_player`):

1. Generate a signing key pair:

   ```bash
   cd cargokit/build_tool
   dart run build_tool gen-key
   ```

2. Put the **public key** in `rust/cargokit.yaml` (`public_key:`) and commit it.
3. Add the **private key** as the repository secret `PRIVATE_KEY`
   (Settings -> Secrets and variables -> Actions). Never commit it.
4. The workflow uses the built-in `GITHUB_TOKEN` with `contents: write` to create
   the release and upload assets.

Whenever the Rust sources / `Cargo.lock` change, the crate hash changes and the
workflow republishes a new release automatically. Keep `rust/Cargo.lock`
committed so the hash matches between CI and consumers.

[cargokit]: https://fzyzcjy.github.io/flutter_rust_bridge/manual/integrate/builtin

## Architecture

```
Dart:  XueHuaPlayerController ──FRB calls──► Rust API (rust/src/api/player.rs)
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

### Regenerating bindings

After changing the Rust API:

```bash
flutter_rust_bridge_codegen generate
```

### Vendored dependency patch

`rust/vendor/irondash_texture` is a local copy of `irondash_texture` 0.5.0 with a
macOS/iOS fix: the upstream backing `IOSurface` uses `bytesPerRow = width * 4`,
which fails Metal's row-alignment requirement on the current Flutter renderer
(`Could not create Metal texture from pixel buffer: CVReturn -6684`). The
vendored copy aligns the stride to 256 bytes and uploads row-by-row. It is wired
in through `[patch.crates-io]` in `rust/Cargo.toml`.

## Troubleshooting

- **`Can't typefind stream` on network video (iOS):** caused by a failed TLS
  handshake when no CA database is configured. This build registers the OpenSSL
  TLS backend and relaxes `ssl-strict`; make sure you are on v1.0.0+.
- **APK too large:** all four Android ABIs are bundled with a large GStreamer
  runtime each. Narrow `abiFilters` or ship per-ABI APKs / an App Bundle.
- **Windows `pkg-config` cannot find `glib-2.0`:** confirm the **development**
  files were installed and `PKG_CONFIG_PATH` points at
  `...\1.0\msvc_x86_64\lib\pkgconfig`.
- **macOS black screen / dylib load failure:** enable
  `com.apple.security.network.client`, and either disable the sandbox (dev) or
  bundle the dylibs and add `disable-library-validation`.

## Maintainers

- Author: **Matkurban** &lt;3496354336@qq.com&gt;
- Repository: <https://github.com/Matkurban/xue_hua_video_player>
- Issues: <https://github.com/Matkurban/xue_hua_video_player/issues>

## License

See [LICENSE](LICENSE).

[`flutter_rust_bridge`]: https://pub.dev/packages/flutter_rust_bridge
[`irondash_texture`]: https://crates.io/crates/irondash_texture
[`signals`]: https://pub.dev/packages/signals
