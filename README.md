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

## Precompiled binaries

The Rust core is built with [cargokit], which supports **precompiled binaries**
so that consumers of this plugin do **not** need the Rust toolchain. When a
consumer builds their app, cargokit computes a `crate-hash` from the Rust
sources (`rust/src/**`, `Cargo.toml`, `Cargo.lock`, `cargokit.yaml`) and
downloads a signed, prebuilt library for that hash from this repo's GitHub
Releases instead of running `cargo`.

> Precompiled binaries only remove the **Rust toolchain** requirement. The
> precompiled artifact still references GStreamer symbols, so iOS/macOS/Linux/
> Windows consumers still need the GStreamer SDK at link/runtime (see the
> per-platform sections below). Android is the exception: the plugin bundles the
> GStreamer runtime, so it needs neither Rust nor the GStreamer SDK.

Behavior:

- Consumers **without** Rust installed automatically use the precompiled binary.
- Consumers **with** Rust installed build from source by default. To force the
  precompiled path (e.g. to reproduce a consumer build), add a
  `cargokit_options.yaml` next to the app's `pubspec.yaml` with:

  ```yaml
  use_precompiled_binaries: true
  ```

Configuration lives in [`rust/cargokit.yaml`](rust/cargokit.yaml) (the download
URL prefix and the ed25519 **public** key used to verify signatures).

### Publishing (maintainers)

Precompiled binaries are produced by
[`.github/workflows/precompile_binaries.yml`](.github/workflows/precompile_binaries.yml)
on every push to `main` (macOS x86_64/arm64, iOS arm64, Linux x86_64, Windows
x86_64, Android arm64) and uploaded to a release tagged `precompiled_<hash>`.

One-time setup in the GitHub repo (`Matkurban/xue_hua_video_player`):

1. Generate a signing key pair:

   ```bash
   cd cargokit/build_tool
   dart run build_tool gen-key
   ```

2. Put the **public key** in `rust/cargokit.yaml` (`public_key:`) and commit it.
3. Add the **private key** as the repository secret `PRIVATE_KEY`
   (Settings → Secrets and variables → Actions). Never commit it.
4. The workflow uses the built-in `GITHUB_TOKEN` with `contents: write` to
   create the release and upload assets.

Whenever the Rust sources / `Cargo.lock` change, the crate hash changes and the
workflow republishes a new release automatically. Keep `rust/Cargo.lock`
committed so the hash matches between CI and consumers.

[cargokit]: https://fzyzcjy.github.io/flutter_rust_bridge/manual/integrate/builtin

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

Because the iOS `GStreamer.framework` is static, its plugins are not
auto-discovered. The Rust core registers the ones needed for playback
(`register_ios_static_plugins()` in `rust/src/player.rs`, guarded by
`#[cfg(target_os = "ios")]`), which is what makes the linker pull the
corresponding `gst_plugin_*_register` objects out of the framework archive. Add
to that list if you need an element from a plugin that isn't registered yet.

### Android (arm64)

Targets arm64-v8a devices. **Consumers need no GStreamer setup**: the plugin
bundles the entire GStreamer runtime. `android/src/main/jniLibs/arm64-v8a/`
ships the umbrella `libgstreamer_android.so` (all of GStreamer + its plugins,
linked statically) and `libc++_shared.so`; these are packaged into the plugin
AAR and merged into the app. The Rust `libxue_hua_video_player.so` is fetched as
a precompiled binary (see "Precompiled binaries"). The Rust core registers the
static plugins itself (`gst_init_static_plugins()` in `rust/src/player.rs`), so
no `GStreamer.init` Java class is required.

A consuming app only has to build for arm64-v8a (e.g. `ndk { abiFilters +=
"arm64-v8a" }`, as in the example). Native-lib extraction is requested by the
plugin's `AndroidManifest.xml` (`android:extractNativeLibs="true"`) and merges
into the app.

- `https://` sources need the openssl gio module's CA bundle; the
  `gstreamer_build` recipe includes `G_IO_MODULES := openssl` but does not yet
  wire the runtime cert path — local files and `http://` work out of the box.

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

Result: `~/Library/Developer/GStreamer/android/1.28.4/arm64/...` (override the
root with `GSTREAMER_ROOT_ANDROID`).

**2. Build `libgstreamer_android.so`** from the recipe in
[`android/gstreamer_build`](android/gstreamer_build) (edit the
`GSTREAMER_PLUGINS` list there to add codecs):

```bash
cd android/gstreamer_build
export GSTREAMER_ROOT_ANDROID="$HOME/Library/Developer/GStreamer/android/1.28.4"
~/Library/Android/sdk/ndk/<ndk-version>/ndk-build \
  NDK_PROJECT_PATH=. NDK_APPLICATION_MK=jni/Application.mk
# → libs/arm64-v8a/libgstreamer_android.so (+ libc++_shared.so)
```

**3. Install the umbrella `.so`** in two places:

```bash
ABI_LIB=~/Library/Developer/GStreamer/android/1.28.4/arm64/lib
JNI=android/src/main/jniLibs/arm64-v8a
cp libs/arm64-v8a/libgstreamer_android.so "$ABI_LIB"/     # for the Rust link step
cp libs/arm64-v8a/libgstreamer_android.so libs/arm64-v8a/libc++_shared.so "$JNI"/  # committed, bundled at runtime
```

- When building the Rust library from source, the `-sys` crates are pointed at
  `libgstreamer_android.so` by our patched cargokit
  (`cargokit/build_tool/lib/src/android_environment.dart`), which sets
  `system-deps` `NO_PKG_CONFIG` overrides for `glib`/`gobject`/`gio`/`gstreamer*`
  (`LIB=gstreamer_android`, `SEARCH_NATIVE=<sdk>/arm64/lib`). This is why the
  `.so` must be copied into the SDK's `arm64/lib` before building. The CI
  precompile workflow does the equivalent by copying the committed
  `jniLibs/arm64-v8a/libgstreamer_android.so` into the SDK lib dir.
- Only arm64-v8a is wired up (`abiFilters` in the plugin/example and the ABI
  trim in `cargokit/gradle/plugin.gradle`). For other ABIs, build the umbrella
  `.so` for that ABI, commit it into the matching `jniLibs` dir, and re-enable
  the ABI in cargokit.

**compileSdk** — some plugins (e.g. `irondash_engine_context` 0.5.5) compile
against an older `compileSdk` than their AndroidX deps require. The example's
[`android/build.gradle.kts`](example/android/build.gradle.kts) forces
`compileSdkVersion 36` on all subprojects to satisfy AAR metadata checks.

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
