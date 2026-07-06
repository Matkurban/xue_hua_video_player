## 1.1.0

### Breaking

- Replaced `irondash_texture` / `irondash_engine_context` with GStreamer VideoOverlay sinks (`glimagesink`, `osxvideosink`, `d3d11videosink`) rendering into Flutter Platform Views.
- Removed `XueHuaPlayerController.textureId`; use `playerId` with `XueHuaVideoView`.
- `createPlayer()` no longer requires `EngineContext`.

### Added

- Platform View factories on Android, iOS, macOS, Windows, and Linux.
- Agent skills configuration (`AGENTS.md`, `docs/agents/*`, `CONTEXT.md`).

## 1.0.19

### Bug fixes

- **Android SIGABRT during `create_player` / `souphttpsrc` (reverts harmful 1.0.18
  changes)**: v1.0.18 used `g_main_context_default()` + `push_thread_default` on a
  new `xhvp-glib` thread and re-registered the OpenSSL GIO TLS backend (already
  registered by `GStreamerInitProvider`), causing `GTlsBackendOpenssl` duplicate
  registration and SIGABRT in our code. v1.0.19 removes those changes and adopts
  the GStreamer Android tutorial model: a dedicated `xhvp-gst` thread with an
  **owned** `GMainContext` (`MainContext::new()`), `MainLoop::run()`, and all
  pipeline / bus-watch / `set_uri` / play / pause / seek operations marshalled
  onto that thread via `g_main_context_invoke`. Flutter texture creation stays on
  the Android main thread.

### Integration

- Network `http(s)://` URIs continue to play directly via `playbin3` /
  `souphttpsrc` (no disk cache, no appsrc workaround).
- Prefer `file://` + non-empty `localPath` when media is already on disk.

## 1.0.18

### Bug fixes

- **Android SIGABRT in `souphttpsrc` after `GStreamerInitProvider` init**:
  `gst_is_initialized()` is often already true at `create_player` because the
  plugin's `GStreamerInitProvider` runs `GStreamer.init(context)` at process
  startup (not because of co-hosted SDKs). v1.0.16 skipped all GLib auxiliary
  setup in that case, so `souphttpsrc` aborted in `g_private_get` on its worker
  thread. When Java has already initialized GStreamer, the plugin now starts a
  dedicated background `GMainLoop` on the default `GMainContext` and registers
  the bundled OpenSSL GIO TLS backend — without calling GLib thread-default APIs
  on the Flutter main thread.

  **Note:** v1.0.18 was superseded by v1.0.19; the default-context approach
  caused new crashes (`xhvp-glib` SIGABRT, duplicate TLS registration).

### Integration

- Network `http(s)://` URIs continue to play directly via `playbin3` /
  `souphttpsrc` (no disk cache, no appsrc workaround).
- Prefer `file://` + non-empty `localPath` when media is already on disk.

## 1.0.17

### Changes

- **Android network playback**: removed HTTP pre-download to disk (`android_http`).
  Network `http(s)://` URIs are passed directly to `playbin3` / `souphttpsrc`
  (streaming playback, no cache file). Before the first HTTP(S) source, the plugin
  ensures Java `GStreamer.init(Context)` on the main thread for TLS/androidmedia
  when sharing GStreamer with another SDK (e.g. `xue_hua_sdk`).

### Integration (IM apps with `xue_hua_sdk`)

- Prefer `file://` + non-empty `localPath` once media is downloaded.
- Network URLs stream via GStreamer directly; the plugin does not download to
  cache first.

## 1.0.16

### Bug fixes

- **Android SIGABRT on main thread with shared GStreamer (`xue_hua_sdk`)**:
  v1.0.15 called `glib::MainContext::ref_thread_default()` during init when
  `gst_is_initialized()` was already true, which aborted in `g_private_get` on the
  Flutter main thread. Shared-runtime init now only syncs the gstreamer-rs
  `INITIALIZED` flag and skips all GLib thread-default / main-loop setup.

### Integration (IM apps with `xue_hua_sdk`)

- Prefer opening video with a local `file://` URI once `received_media` reports
  a non-empty `localPath`; do not call `open()` while `state=0` and
  `localPath=` is empty.

## 1.0.15

### Bug fixes

- **Android SIGABRT in `souphttpsrc` with shared GStreamer (`xue_hua_sdk`)**:
  `create_player` now succeeds, but HTTP playback crashed in
  `g_main_context_push_thread_default` when another SDK had already called
  `gst_init`. When shared GStreamer is detected, `http(s)://` sources are
  downloaded through the Android network stack into the app cache and played via
  `file://` instead of `souphttpsrc`. Also starts a GLib main-loop thread and
  registers the OpenSSL GIO TLS backend when reusing an existing runtime.

## 1.0.14

### Bug fixes

- **Android SIGABRT on FRB worker thread**: `android_prepare_sendable_texture_sender`
  used `thread_local` storage, which aborts when first touched from flutter_rust_bridge's
  threadpool (`Thread-8`). The `MainThreadSender` is now passed through the
  `send_and_wait` closure into `into_sendable_texture_with_sender` instead.

## 1.0.13

### Bug fixes

- **Android SIGABRT on `create_player` (nested main-looper)**: when
  `GstPlayer::new` ran on the platform main thread it called
  `RunLoop::sender_for_main_thread()`, which invokes `ALooper_prepare` and
  conflicts with Flutter's main looper. Texture creation is now inline on
  Android main; the worker thread stashes a `MainThreadSender` before hopping to
  main so `into_sendable_texture` never calls `RunLoop::current()` on the UI
  thread.

## 1.0.12

### Bug fixes

- **Android IM / multi-SDK (`xue_hua_sdk`)**: create Flutter texture before
  `gst::init` so JNI/SurfaceProducer setup is not blocked by GStreamer startup.
  When `libgstreamer_android.so` is already initialized by another in-process SDK,
  sync gstreamer-rs via `gst_is_initialized()` instead of calling `gst_init`
  again. All critical steps log directly to logcat (`xue_hua_video_player` tag)
  without relying on the global `log` logger.

## 1.0.11

### Bug fixes

- **Android SIGABRT when another SDK owns `log`**: drop `android_logger::init_once`
  (which conflicted with host apps like IM clients using their own logger). Panic
  backtraces now go directly to logcat via `__android_log_write` (`xue_hua_video_player`
  tag).
- **Android `create_player` on FRB worker thread**: the full player setup
  (`GstPlayer::new`, texture `SurfaceProducer`, GStreamer pipeline) now runs on
  the platform main thread. **Requires fresh precompiled `libxue_hua_video_player.so`**.

## 1.0.10

### Bug fixes

- **Android SIGABRT on `create_player` (SurfaceProducer path)**: vendored
  `irondash_texture` no longer panics when `ANativeWindow` buffer geometry does
  not exactly match the RGBA payload (`assert!` replaced with graceful error
  handling and row-by-row copy). `ANativeWindow_lock` failures are checked.
  `onSurfaceAvailable` now refreshes the native window handle. `SendableTexture`
  uses `Capsule::new_with_sender` so cross-thread drops do not abort. Android
  panic messages and backtraces are logged to logcat (`xue_hua_video_player` tag).
  **Requires fresh precompiled `libxue_hua_video_player.so`** for the new
  `crate-hash`.

### Tooling

- Release builds embed debug symbols (`[profile.release] debug = true`) for
  `ndk-stack` / `llvm-addr2line` symbolication.
- Added [`scripts/symbolicate_android_tombstone.sh`](scripts/symbolicate_android_tombstone.sh)
  and [`example/android/cargokit_options.yaml`](example/android/cargokit_options.yaml)
  (`use_precompiled_binaries: false`) for local crash diagnosis.

## 1.0.9

### Bug fixes

- **Android release SIGABRT (legacy texture path)**: vendored `irondash_texture`
  no longer silently falls back to `createSurfaceTexture()` when
  `createSurfaceProducer()` is available but fails. `ANativeWindow` acquisition
  is deferred until the first frame on the SurfaceProducer path. Adds
  `IrondashSurfaceProducerCallback` for surface lifecycle and calls
  `scheduleFrame()` after posting pixels. **Requires fresh precompiled
  `libxue_hua_video_player.so`** for the new `crate-hash`.

## 1.0.8

### Bug fixes

- **Android release playback / crash**: extend consumer ProGuard rules to keep
  `org.freedesktop.gstreamer.**` JNI helper classes (e.g.
  `GstAmcOnFrameAvailableListener`). R8 was removing them in minified release
  builds, causing `ClassNotFoundException` and GStreamer MediaCodec failures.

## 1.0.7

### Bug fixes

- **Android release SIGABRT on video page**: vendored `irondash_texture` now uses
  Flutter's `createSurfaceProducer()` API instead of legacy `createSurfaceTexture()`,
  fixing `Fatal signal 6 (SIGABRT)` on modern Flutter engines (Impeller/Vulkan).
  Falls back to `createSurfaceTexture` on older Flutter versions. **Requires a
  fresh native build** (precompiled `libxue_hua_video_player.so` must be rebuilt
  for the new `crate-hash`).

## 1.0.6

### Bug fixes

- **Android release crash**: ship consumer ProGuard rules so R8/minify builds no
  longer strip `IrondashEngineContextPlugin.getTextureRegistry()` (fixes
  `NoSuchMethodError` when opening a video page in release).

## 1.0.5

### Improvements

- **Smaller macOS app bundles (Slim Runtime)**: vendored `GStreamer.framework` is
  now stripped (no `bin/` / devel dirs), pruned to a playback plugin whitelist
  (~80 plugins vs 257), and orphan dylibs removed. Typical universal apps drop
  from ~680MB to ~350–450MB GStreamer footprint.
- **applemedia hardware decode on macOS**: bundled `applemedia` plugin is kept in
  the whitelist; `GST_PLUGIN_FEATURE_RANK` prefers VideoToolbox (`vtdec`) with
  `libav` as fallback.
- **Per-architecture macOS builds**: set `XUE_HUA_GSTREAMER_ARCH=arm64` or
  `x86_64` (default `universal`) before `pod install` to thin the embedded
  framework for single-arch distribution.

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
