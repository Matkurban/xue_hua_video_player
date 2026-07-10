## 1.4.1

### Bug fixes

- **Android portrait video tiny in center**: emit `VIDEO_SIZE` /
  `METADATA_CHANGED` from negotiated `glimagesink` sink-pad caps (pad probe +
  post-PAUSED query) so Dart `aspectRatio` is not stuck on the 16:9 fallback.
  Set `glimagesink` `force-aspect-ratio=false` so Dart `FittedBox` owns
  fit/fill/stretch and native does not double-letterbox into a landscape
  SurfaceProducer.

## 1.4.0

### Breaking changes

- **Native stack**: replace Rust `flutter_rust_bridge` + `gstreamer-rs` with a
  **thin C player core** (`native/`) driven by **Dart FFI**. Dart public API
  (`XueHuaPlayerController`, `PlaybackSession`, `PlayerCommandPort`) is unchanged.
  Consumers no longer need a Rust toolchain; platform builds link GStreamer SDK
  binaries and compile `native/` instead of cargokit.
- **GStreamer patches**: when upstream C must change, use
  [Matkurban/gstreamer](https://github.com/Matkurban/gstreamer) as the patch
  source (`XHVP_GSTREAMER_SRC`); see `third_party/gstreamer.md`.

### Features

- C ABI `xhvp_player_*` / `xhvp_texture_*` with dedicated GMainContext thread,
  `playbin3` URI playback, appsink BGRA frames (desktop/Apple), Android
  `glupload` → `glcolorconvert` → `gltransformation` → `glimagesink`
  VideoOverlay via existing SurfaceProducer JNI bridge.
- **Swift Package Manager** support for iOS and macOS (dual CocoaPods + SPM),
  with plugin sources under `ios|macos/xue_hua_video_player/`.
- Android plugin module converted to Kotlin DSL (`build.gradle.kts` /
  `settings.gradle.kts`), modern AGP (`minSdk 24`, Java 17).
- **Android Java package**: `com.flutter_rust_bridge.xue_hua_video_player` →
  `xue_hua.video_player` (Gradle namespace, Manifest, ProGuard, JNI symbols).
  Dart pub package name remains `xue_hua_video_player`.
- Example iOS/macOS apps are SPM-only (CocoaPods deintegrated); macOS embeds
  GStreamer via an Xcode Run Script.

### Bug fixes

- **Android abandoned Surface / no picture after enter preview**: texture
  lifetime now follows the player (`disposeTexture` from
  `PlaybackSession.dispose`), not `TextureVideoSurface` dispose — Hero /
  SignalBuilder remounts no longer `ImageReader_close` the 1×1 producer mid-
  PLAYING. Eager DisplayMetrics 16:9 `setSize` on texture create; cleanup
  always clears then posts a rebind via `getSurface()`.
- **Android stuck PAUSED when layout `setSize` never runs** (IM Hero / late
  texture id): revert defer-PLAYING-until-buffer>1×1 (deadlocked with no
  `ImageReader` resize). Sync Android buffer size immediately after
  `createTexture`; fall back to MediaQuery-fitted 16:9 when layout size is
  null/unusable. Pass `null` instead of `Size.zero` from presentation.
- **Android black screen after layout `setSize`**: apply the new
  `ANativeWindow` with `invoke_sync`. Cleanup now clears+rebinds instead of
  skipping when the bound `Surface` still looks valid.
- **Material controls overflow on narrow/Hero widths**: Flexible+FittedBox
  time label; hide loop/speed under 280px (keep mute + fullscreen).
- **Android SIGABRT on layout `setSize` (vivo / Android 16)**: unbind
  VideoOverlay and release `ANativeWindow` **synchronously** before
  `SurfaceProducer.setSize` / `release`. The prior skip-cleanup-while-resizing
  fix left `glimagesink` holding a destroyed Surface →
  `eglCreateWindowSurface` → FORTIFY destroyed-mutex abort. Skip redundant
  rebinds of the same `Surface` instance.
- **Android squashed video height**: size `SurfaceProducer` to the fitted
  video rectangle (video aspect via `applyBoxFit` / cover scale), not the raw
  Stack viewport. Portrait viewport + 16:9 Texture stretched the buffer and
  compressed height.
- **Android black picture (audio OK, position advances)**: drive
  `SurfaceProducer.setSize` from layout (not the FittedBox unit box
  `SizedBox(height: 1)`). Syncing from Texture constraints left ImageReader at
  1×1 — black when scaled. Per Flutter `SurfaceProducer.setSize` (physical
  pixels).
- **Android unable to play (playing but no A/V / position stuck)**: restore the
  MediaCodec GL bridge in the C video-sink bin —
  `glupload` → `glcolorconvert` → `gltransformation` → `glimagesink`. The
  Rust→C port had dropped `glupload`/`glcolorconvert`, so `amcvideodec` failed
  caps negotiation (HEVC/AVC allocate→release thrash) and playbin stalled
  despite `PlayerState.playing`. Do not reintroduce `gldownload` on this path.
- **Android unable to play (no A/V)**: restore early SurfaceProducer bind
  (including default 1×1) and real `gst_element_set_state(PLAYING)` whenever an
  `ANativeWindow` exists. Prior size>1 / no-1×1 gates left `android_window==0`
  so `glimagesink` stalled the whole playbin (silence + black). Double
  `gst_video_overlay_expose` on apply; Dart no longer fakes `playing` at
  buffering 100% while native is still buffering.
- **Android black screen (1×1 ImageReader)**: drive `SurfaceProducer.setSize`
  from Flutter layout physical pixels (`syncTextureSize`), then re-bind the
  overlay; prefer live `ANativeWindow` dimensions in `apply_overlay`. Do not
  size from decoded video caps. Skip `onSurfaceCleanup`→destroy while resizing
  (setSize race cleared the new window).
- **Android no video / false load -1**: keep `ANativeWindow` across media
  reload (`destroy` only unbinds VideoOverlay; release on surface
  cleanup/dispose only). SurfaceProducer does not re-fire
  `onSurfaceAvailable` on reload. Drain the GST main context after a failed
  preroll `get_state`, then return OK while the pipeline still exists so Dart
  does not tear down a usable session; schedule deferred play whenever a
  window is held.
- **Android false load -1 / stream error**: ignore child-element
  `GST_MESSAGE_ERROR` during codec autoplug; Android `load_uri` returns OK
  after PAUSED and starts play asynchronously; widen usable-state check on
  `set_state` FAILURE; keep asset temp file if pipeline still exists (avoids
  `Internal data stream error` from premature unlink).
- **Android load crash / false -1**: copy event `message` into a durable
  player buffer before async Dart `NativeCallable.listener` (fixes
  `FormatException` on stack ERROR strings); treat `set_state` FAILURE as OK
  when pipeline is already PAUSED/PLAYING after codec autoplug; harden event
  UTF-8 decode in `event_pump.dart`.
- **iOS startup GLib/ORC warnings**: port sandbox env setup before `gst_init`
  (`native/src/apple_env.c`: `ORC_CODE=backup`, writable `HOME`/`XDG_*`/
  `GST_REGISTRY`). Fixes `g_dir_open_with_errno`, `g_filename_to_utf8`, and
  ORC W+X mmap failures under Hardened Runtime.
- **iOS HTTPS network video**: register GIO OpenSSL TLS backend
  (`native/src/ios_tls.c`) and configure `souphttpsrc` via playbin
  `source-setup` (`ssl-strict=false`, UA). Fixes
  `Stream doesn't contain enough data` / `load_uri` `-1` after Rust→C migration.
- **Android `ANativeWindow` lifecycle**: retain/release on surface bind/clear;
  surface ops look up player by id on the GST thread.
- **Safe pipeline destroy**: wait for `GST_STATE_NULL` before unref; clear
  appsink callbacks; guard frame callback after teardown.
- **FFI serialization**: replace per-call `Isolate.run` with one long-lived
  worker isolate command queue (avoids dispose/load races and UI freezes).
- **SurfaceProducer**: bind using producer width/height only; remove
  caps→`setSize` / `setTextureContentSizeSync` path.
- **Overlay / pause**: cache overlay element; clear `pending_auto_play` on
  pause/stop; restore consumer ProGuard keeps for GStreamer + JNI classes.
- **Tracks**: free `select_streams` GList; mark `selected` from
  `STREAMS_SELECTED`; marshal GST ops by player id; null `playerId` early on
  session dispose.
- playbin3 track listing via `GstStreamCollection` (no `n-audio` /
  `current-audio` GObject CRITICAL spam).
- Buffering state machine: keep UI out of sticky `buffering` when percent
  reaches 100; avoid READY/PAUSED clobbering during download rebuffer.
- Asset load failures surface as Dart `StateError` with the asset key.
- **Asset temp file**: `g_file_open_tmp` template must end with `XXXXXX`
  (was `xhvp-asset-XXXXXX.mp4`, which always failed).
- **Playback speed**: rate seeks use `KEY_UNIT` (not `ACCURATE`), check seek
  result, defer while buffering, and attach `scaletempo` audio-sink when
  available so pitch stays natural.
- **Video rotation**: apply via GStreamer video-sink bin (`videoflip` /
  `gltransformation`); Dart presentation no longer wraps Texture in
  `RotatedBox` (avoids black frames).
- **Playback speed jump-to-end / color shift**: do not use flushing
  `SEEK_TYPE_NONE` (playbin3 can land at EOS) or `INSTANT_RATE_CHANGE` (can
  corrupt videoconvert/appsink colors). Flush seek to queried/cached position
  with `SEEK_TYPE_SET`; desktop sink pins BGRA via capsfilter before appsink.
- **Rotation aspect**: keep DAR in sync with post-`videoflip` width/height so
  90°/270° letterbox correctly.
- **EOS replay**: `play()` after completed seeks to 0 (was resuming near EOS).
- **Seek buffering**: clear sticky buffering after seek; Dart no longer sets
  optimistic buffering on scrub.
- **UI freeze**: blocking FFI transport runs off the UI isolate.


## 1.3.0

### Breaking

- **Android GStreamer runtime is no longer pre-committed** under
  `android/src/main/jniLibs/`. Each Android build downloads the official
  GStreamer Android SDK (if missing) and runs **ndk-build** to produce
  `libgstreamer_android.so` per ABI. The **first build needs network access**;
  later builds reuse the user cache
  (`~/Library/Caches/xue_hua_video_player/gstreamer/android/<GST_VER>/` on macOS).
  Override with `GSTREAMER_ROOT_ANDROID`, `GST_VER` (default `1.28.4`), or
  `XUE_HUA_GSTREAMER_ROOT`. See `android/scripts/` and the README Android
  section.

### Added

- Android Gradle tasks `ensureGstreamerAndroid` and `buildGstreamerUmbrella`,
  wired into cargokit Rust builds and native-lib packaging.
- Shell helpers: `android/scripts/ensure_gstreamer_android.sh`,
  `build_gstreamer_umbrella.sh`, `gstreamer_paths.sh`.
- Android HW-decode video path stays end-to-end GL:
  `glupload → glcolorconvert → gltransformation → glimagesink` (URI playbin and
  AppSrc branches).
- `make_orientation_element()` on Android creates `gltransformation` for
  in-pipeline rotation; non-Android platforms still use `videoflip`.
- `souphttpsrc` HTTP status logging: warns on non-2xx responses (Android
  logcat via `diag::logcat_error`).

### Bug fixes

- **Android network video `not-negotiated` / `Internal data stream error`**:
  GL bridge for MediaCodec (`amcvideodec`) texture output in the playbin
  video-sink bin.
- **Android network video black screen (audio only)**: removed `gldownload`,
  `videoconvert`, and CPU `videoflip` from the External OES path — they cannot
  map HW decoder surfaces and dropped every frame.
- **Android startup SIGABRT** after the GL rotation change: `gltransformation`
  `rotation-z` is GObject `gfloat` (`f32`); reading/writing it as `f64` panicked
  in glib-rs during pipeline construction.

### Platform notes (Android)

- `android.ndkVersion` must be set in the consuming app's `build.gradle` (the
  example already does).
- `GStreamerInitProvider` still loads `gstreamer_android` and calls
  `GStreamer.init(context)` before Rust registers plugins.

## 1.2.0

### Breaking

- Removed Cargokit precompiled Rust binary support. The plugin now always
  compiles `libxue_hua_video_player` from source during the app build. Consumers
  need the Rust toolchain (`rustup`) on the machine that builds the app.
  Precompiled release downloads, `rust/cargokit.yaml`, `cargokit_options.yaml`
  (`use_precompiled_binaries`), and the `precompile_binaries` CI workflow are
  gone.
- **`XueHuaVideoView`**: removed `BoxFit fit`; use `AspectRatioMode aspectRatioMode`
  (default `AspectRatioMode.fit`). Maps to the Rust pipeline sink (`force-aspect-ratio`).

  ```dart
  // Before
  XueHuaVideoView(controller: c, fit: BoxFit.cover)

  // After
  XueHuaVideoView(controller: c, aspectRatioMode: AspectRatioMode.fill)
  ```
- **All platforms** now render through Flutter external **`Texture`** widgets
  instead of Platform Views or desktop overlay windows.
- Removed `VideoSurfaceKind.platformView` and `VideoSurfaceKind.desktopOverlay`.
- Removed native Platform View factories (iOS/macOS/Android) and Windows/Linux
  desktop overlay MethodChannel (`xue_hua_video_player/desktop_overlay`).
- Removed **`buildXueHuaVideoPlatformView`** and **`lib/src/platform_view.dart`**
  (unused one-line alias over **`buildVideoSurface`**). Use **`XueHuaVideoView`**
  (`showControls: false` for custom chrome) or package-private **`PlaybackPresentation`**.

### Added

- Custom zero-dependency texture bridge: GStreamer **`appsink`** (BGRA) on
  Apple/Windows/Linux; Android **`SurfaceProducer`** + **`glimagesink`**
  VideoOverlay for zero-copy GL rendering.
- Native `xue_hua_video_player/texture` MethodChannel on all platforms
  (`createTexture` / `disposeTexture`).
- Rust C-ABI frame pull API (`xhvp_texture_*`) for desktop/Apple pixel-buffer
  textures.
- **`lib/src/surface/`** (package-private): `VideoSurfaceHandle`, `VideoSurfaceKind`,
  and `Texture` surface builders.
- **`XueHuaPlayerController`** implements **`PlaybackControlsModel`** for the
  built-in control bar.
- Widget/unit tests for surface handle routing, aspect-ratio sync, and overlay
  lifecycle.
- **`test/support/`** test seam: `FakePlayerCommandPort`, `PlayerEventFixtures`, and
  `FakePlaybackControlsModel`. Dart/Rust boundary tests mock **`PlayerCommandPort`**
  (not `RustLib.initMock`).

### Bug fixes

- **Android black screen (audio only)**: sync `SurfaceProducer` content size from
  decoded video caps via `setContentSize` so `ImageReader` is not stuck at 1×1;
  JNI surface bind no longer passes stale default dimensions to GStreamer.
- **Android playback never starting**: restore eager `bindSurfaceIfAvailable()`
  after `setCallback` on texture create (Flutter does not always fire
  `onSurfaceAvailable` on first attach).
- **iOS network URL — loading spinner / controls stuck**: treat the texture/appsink
  path as overlay-ready so playbin re-buffering at 100% emits `Playing` instead
  of leaving UI in `Buffering` forever (assets were unaffected).
- **Playback engine deadlocks** during overlay session attach and shell locking.
- **EOS replay / loop**: preserve playback rate when restarting from end-of-stream.

## 1.1.0

### Breaking

- Replaced `irondash_texture` / `irondash_engine_context` with GStreamer VideoOverlay sinks (`glimagesink`, `osxvideosink`, `d3d11videosink`) rendering into Flutter Platform Views.
- Removed `XueHuaPlayerController.textureId`; use `playerId` with `XueHuaVideoView`.
- `createPlayer()` no longer requires `EngineContext`.

### Added

- Platform View factories on Android, iOS, macOS, Windows, and Linux.
- Agent skills configuration (`AGENTS.md`, `docs/agents/*`, `CONTEXT.md`).

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
