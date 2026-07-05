## 1.0.4

### Bug fixes

- Fixed macOS apps crashing at launch with
  `dyld: Library not loaded: @rpath/GStreamer.framework/...` when consumers did
  not manually add the Podfile embed helper. GStreamer is now embedded
  automatically via CocoaPods `vendored_frameworks` and `[CP] Embed Pods
  Frameworks` — **no Podfile changes required** (requires **v1.0.4+**).

## 1.0.3

### Features

- **macOS App Sandbox / Mac App Store**: the plugin now embeds the official
  universal `GStreamer.framework` into the `.app` at build time and configures
  `GST_PLUGIN_SYSTEM_PATH`, `GIO_MODULE_DIR`, and a sandbox-writable
  `GST_REGISTRY` before `gst::init()` (`setup_macos_env()` in the Rust core).
  Consumers no longer need manual Xcode Copy Files steps.

### Improvements

- **macOS GStreamer auto-setup**: on first `pod install`, the official runtime +
  devel SDK is downloaded automatically into the user cache
  (`~/Library/Caches/xue_hua_video_player/gstreamer/<version>/`) — no `sudo`
  or manual `setup_gstreamer_macos.sh` required for consumers.
- **Smaller macOS app bundles**: the build cache keeps the full SDK for linking,
  but only a **runtime snapshot** (`GStreamerRuntime.framework`) is embedded into
  the final `.app`, stripping static libraries (`.a`) and devel-only directories.
  Typical release apps are ~700 MB instead of ~4 GB.
- macOS podspec links the official framework via `system-deps` + `-framework
  GStreamer` (aligned with iOS), supporting universal (x86_64 + arm64) builds.
- CI precompile jobs use the cache-based ensure script instead of a system-wide
  `sudo installer`.

### Platform notes (macOS)

- Enable App Sandbox and `com.apple.security.network.client` in
  `macos/Runner/*entitlements` for network playback.
- Local Homebrew-only debugging remains available via
  `XUE_HUA_ALLOW_HOMEBREW_GSTREAMER=1` (not suitable for App Store submission).
- Optional overrides: `XUE_HUA_GSTREAMER_ROOT`, `GSTREAMER_FRAMEWORK_SRC`,
  `GSTREAMER_RUNTIME_FRAMEWORK_SRC`. Maintainers may still run
  `sh tool/setup_gstreamer_macos.sh --system` for a system-wide install.

## 1.0.2

### Bug fixes

- Fixed Cargokit precompiled binary downloads failing on macOS (and other
  platforms) with `ClientException: Connection closed while receiving data`.
  Downloads now retry transient network errors up to 10 times with exponential
  backoff, stream large artifacts to disk, and gracefully fall back to a local
  Rust build when the download still fails.

## 1.0.1

### Bug fixes

- Fixed replay after playback reaches the end of the media: pressing play again
  now seeks back to the start instead of staying stuck at the end.
- Buffering events now set `PlayerState.buffering` so the control bar shows the
  loading indicator during network buffering.

### Improvements

- Playback speed changes now preserve the original pitch (via GStreamer's
  `scaletempo` audio filter) instead of shifting tone at non-1x rates.
- Optimized control bar UI.

### Platform notes

- Android: bundled `audiofx` plugin (includes `scaletempo`) in
  `libgstreamer_android.so` so pitch-preserving speed works out of the box.
- Other platforms: pitch-preserving speed uses the system `audiofx` / `plugins-good`
  package (`gstreamer1.0-plugins-good` on Linux, GStreamer MSVC good plugins on
  Windows, Homebrew `gst-plugins-good` on macOS). If `scaletempo` is unavailable,
  playback speed falls back to the previous pitch-shifting behaviour.

## 1.0.0

Initial release.

### Features

- Cross-platform video playback (Android, iOS, macOS, Windows, Linux) powered by
  GStreamer through a Rust `flutter_rust_bridge` core.
- GPU texture rendering via `irondash_texture` (zero-copy where possible).
- `XueHuaPlayerController` with reactive `signals` state: playback state,
  position, duration, video size, aspect ratio, buffering percent, volume, speed,
  looping, muted, `isPlaying`, `isCompleted`, and error.
- Playback controls: open / play / pause / stop / seek / volume / mute / speed /
  looping, plus `queryPosition` / `queryDuration`.
- Media sources: network URLs, local files, and Flutter assets (`VideoSource`).
- `XueHuaVideoView` widget with a built-in, auto-hiding, themeable control bar
  (adaptive / Material / Cupertino) via `VideoControlsTheme`.
- Precompiled Rust binaries through cargokit, so consumers need no Rust toolchain.
- Android bundles the full GStreamer runtime for all four ABIs (`arm64-v8a`,
  `armeabi-v7a`, `x86`, `x86_64`); no GStreamer setup required on the consumer
  side.

### Platform notes

- iOS: registers the static GStreamer plugins and the OpenSSL TLS backend, and
  prepares the runtime environment (`ORC_CODE=backup`, sandbox-writable
  `HOME`/`TMPDIR`/`XDG_*`) before `gst::init()`, fixing startup GLib warnings, the
  ORC JIT error under the Hardened Runtime, and the `Can't typefind stream`
  failure on HTTPS network video.
- HTTPS uses `ssl-strict = false` to maximize compatibility (server certificate
  verification is skipped); see the README security note.
- Vendored `irondash_texture` patch aligning the backing `IOSurface` stride to
  256 bytes to satisfy Metal's row-alignment requirement on macOS/iOS.
