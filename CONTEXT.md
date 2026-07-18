# xue_hua_video_player — Domain Context

Cross-platform Flutter video player plugin. Decoding via GStreamer (**native C core** + **Dart FFI**); rendering via Flutter external **`Texture`** widgets backed by a custom native bridge (no third-party texture plugins).

## Core components

| Term | Meaning |
|------|---------|
| `native/` C core | `xhvp_player_*` ABI: pipeline, GMainContext thread (`xhvp-gst`), bus events, appsink frames / Android VideoOverlay |
| `FfiPlayerCommandPort` | Dart production seam (`lib/src/player/ffi_command_port.dart`) over `dart:ffi` |
| `FfiNativeWorker` | Long-lived isolate queue for blocking `xhvp_player_*` transport (create/events stay on root) |
| `XueHuaPlayerController` | Dart **public facade** (thin): delegates to **`PlaybackSession`** |
| `PlaybackSession` | Dart **deep orchestration module**: signals, events, `open()`, transport |
| `PlayerCommandPort` | Dart/native seam; prod = `FfiPlayerCommandPort`; tests use `FakePlayerCommandPort` |
| `playbin3` | GStreamer high-level playback element (URI in, A/V out) |
| `VideoOverlay` | GStreamer interface for Android `glimagesink` → `ANativeWindow` |
| Matkurban/gstreamer | Upstream **patch source** when Gst C must change (`XHVP_GSTREAMER_SRC`); see `third_party/gstreamer.md` |

## Platform video sinks (current)

| Platform | Sink | Flutter integration |
|----------|------|---------------------|
| Android | `glupload` → `glcolorconvert` → `glvideoflip` → `tee` → `glimagesink` (+ capture: `gldownload` → `appsink`) | `TextureRegistry.SurfaceProducer` → `Surface` → `ANativeWindow` (VideoOverlay); frames via tee appsink |
| iOS / macOS | `appsink` (BGRA) | `FlutterTexture` + IOSurface-backed `CVPixelBuffer` |
| Windows / Linux | `appsink` (BGRA) | `PixelBufferTexture` / `FlPixelBufferTexture` (RGBA upload) |

## Rendering model

- **Apple + desktop:** GStreamer terminates in `appsink`; C `frame.c` double-buffers BGRA and exposes `xhvp_texture_*` for native texture plugins.
- **Android display:** `glupload → glcolorconvert → glvideoflip → tee → glimagesink`
  (VideoOverlay / SurfaceProducer). The `glupload`/`glcolorconvert` bridge is
  required for MediaCodec (`amcvideodec`) GLMemory / external-OES negotiation.
- **Android capture branch:** same tee also feeds
  `gldownload → videoconvert → appsink` so `frame.c` /
  `xhvp_player_capture_frame` work without changing the display path.
- **Thumbnails:** `xhvp_thumbnail_capture` builds a short-lived headless playbin
  (no player slot / no Texture).
- Dart embeds video with the Flutter `Texture` widget; MethodChannel `xue_hua_video_player/texture`.

## GStreamer runtime

- Dedicated **`xhvp-gst`** thread owns a non-default `GMainContext` and runs `g_main_loop_run`.
- Pipeline ops are marshalled onto that thread via `xhvp_runtime_invoke_sync` / `_async`.
- Android: `GStreamerInitProvider` (`xue_hua.video_player`) loads
  `gstreamer_android` then `xue_hua_video_player`, runs `GStreamer.init(Context)`
  on the main thread, then `NativeRuntimeWarmup` → `xhvp_init` on a **background**
  thread (not from `JNI_OnLoad`). Dart `XueHuaVideoPlayer.initialize()` is
  kickoff-only (opens dylib, starts `xhvp_init_async` / worker; under 50ms).
  `ensureReady()` awaits full readiness; `create` / `captureThumbnail` call it
  so the UI isolate is not blocked on `gst_init`.
- Do **not** block the Flutter UI isolate with `g_main_loop_run`.
- Do **not** call blocking `get_state` from bus watch callbacks.

## Android VideoOverlay

- Own `ANativeWindow` refs from `ANativeWindow_fromSurface`; release on
  replace / surface cleanup / dispose — **not** on every media `destroy`/`load`
  (SurfaceProducer does not re-fire `onSurfaceAvailable` on reload).
- Pipeline reload only unbinds VideoOverlay from the old playbin, then
  re-applies the retained window handle to the new sink.
- Surface ops pass `XhvpPlayerId` and re-lookup on `xhvp-gst` (ignore if disposed).
- Bind immediately after `createSurfaceProducer`. Prefer an eager
  DisplayMetrics contain-fit 16:9 `setSize` so the first window is not stuck
  at the default 1×1 until Dart layout sync runs. Without any
  `ANativeWindow`, `glimagesink` stalls the whole playbin (audio included) —
  see GStreamer Android tutorial 3.
- **Texture lifetime follows the player**, not the view: `createTexture` on
  first mount; `disposeTexture` from `PlaybackSession.dispose` (or playerId
  swap). Do not release on `TextureVideoSurface` dispose — Hero /
  SignalBuilder remounts would close the ImageReader mid-playback
  (`BufferQueue has been abandoned` / `EGL_BAD_SURFACE`).
- Bind/unbind from SurfaceProducer lifecycle (`onSurfaceAvailable` / cleanup / dispose).
  Pass producer width/height into overlay.
  **Do** drive `SurfaceProducer.setSize` from the **fitted video rectangle**
  (same aspect as the Texture / `SizedBox(width: ratio, height: 1)`), via
  `TextureVideoSurface.androidLayoutSize` from an outer `LayoutBuilder` +
  `applyBoxFit` (contain) or cover-scale. Do **not** pass the raw Stack
  viewport: a portrait viewport buffer mapped into a 16:9 Texture squashes
  height. The FittedBox unit child is intentionally `height: 1` and must not
  be used as the sync source either.
  This is not a prerequisite for entering PLAYING; a sized window (eager or
  layout) is enough to `set_state(PLAYING)`. Layout `setSize` refines pixels.
  **Do not** drive `setSize` from decoded video caps.
  **Before** `SurfaceProducer.setSize` / `release`, synchronously unbind
  VideoOverlay and release the `ANativeWindow` (`xhvp_player_clear_android_surface`
  uses `invoke_sync`). Skipping only the cleanup→destroy while `resizing` is not
  enough: `setSize` destroys the old Surface while `glimagesink` still holds it
  → FORTIFY `pthread_mutex_lock` on a destroyed mutex in `eglCreateWindowSurface`.
  While `setSize` runs, ignore `onSurfaceCleanup` so it cannot clear the newly
  bound window on the GST queue. On cleanup (when not resizing), always clear
  then **post** a rebind via `getSurface()` so Flutter can recreate the
  ImageReader without waiting for Activity `onResume`.
  Surface notify uses `invoke_sync` so GST holds the new window before Java
  returns from bind.
  Skip redundant `ANativeWindow_fromSurface` when the same `Surface` instance and
  producer size are already bound.
  After `createTexture`, always sync size from Dart; if layout size is unusable
  (Hero / zero viewport), fall back to a MediaQuery-fitted 16:9 rect.
- Defer PLAYING only when `android_window == 0`; a 1×1 window is enough to
  `set_state(PLAYING)`. Clear `pending_auto_play` on pause/stop.
  When a window is already held at `load_uri`, schedule deferred play after PAUSED.
  On `apply_overlay`, call `gst_video_overlay_expose` twice (GLES size propagation).
- Android `load_uri` must not return `-1` solely because preroll `get_state`
  raced ahead of bus updates; drain the GST context and keep OK while the
  pipeline exists.
- On `apply_overlay`, prefer live `ANativeWindow_getWidth/Height` over a stale
  first-bind cache for the VideoOverlay render rectangle.
- HW decode path: keep end-to-end GL
  (`glupload` → `glcolorconvert` → `glvideoflip` → `glimagesink`); do not
  insert `gldownload` / CPU `videoconvert` / `videoflip` on that path.

## Apple packaging (CocoaPods + SwiftPM)

- iOS/macOS plugin sources live under `ios|macos/xue_hua_video_player/` (`Package.swift` + `Sources/` + `NativeCore/`).
- **`native/` is the canonical C tree.** `ios|macos/.../NativeCore/{include,src}` are **synced real copies** (not symlinks): `dart pub publish` turns directory symlinks into path-text stubs, which leaves SPM’s `xhvp_player_c` empty and causes undefined `_xhvp_*` at link. After editing `native/`, run [`tool/sync_native_core.sh`](tool/sync_native_core.sh); before publish run [`tool/verify_native_core.sh`](tool/verify_native_core.sh).
- Plugin podspecs remain for host apps that still use CocoaPods (`build_pod.sh` + `-force_load`); CocoaPods still compiles from `native/` directly.
- CocoaPods injects `STRIP_STYLE=non-global` and Runner `-force_load` via `user_target_xcconfig` so Dart `DynamicLibrary.process()` / `dlsym` can resolve `xhvp_*` in Release/Archive.
- SPM hosts (including this repo’s **example**) must set Runner Strip Style to **Non-Global Symbols** themselves; see README “Apple Release / FFI symbols”.
- Under SPM, CocoaPods `vendored_frameworks` does **not** embed GStreamer. Hosts with a `macos/Podfile` must call `install_gstreamer_embed_script!` from [`macos/gstreamer_podfile_helper.rb`](macos/gstreamer_podfile_helper.rb) in `post_install` (see README). Pure-SPM example uses a Runner Run Script (`embed_gstreamer_framework.sh`).
- `xhvp_ffi_retain_symbols()` (called from plugin `register`) keeps Dart-looked-up ABI symbols from dead-strip.
- The **example** app is SPM-only (no `Podfile`); macOS Runner embeds GStreamer via an Xcode Run Script phase (`macos/scripts/embed_gstreamer_framework.sh`).
- After editing `native/` (and syncing NativeCore), clean the example (`flutter clean`) so SPM/Pods recompile C; do not run from a stale copy under another path (e.g. `VideoPlayer/` vs `XueHuaPackages/`).

## Presentation layout (Dart)

- Aspect ratio modes (`fit` / `fill` / `stretch`) are applied in
  `PlaybackPresentation` via `FittedBox`. Dart owns letterboxing; Android
  `glimagesink` uses `force-aspect-ratio=false` so the sink fills the buffer
  (native must not letterbox again into a landscape SurfaceProducer).
  The FittedBox child is a unit `SizedBox(width: ratio, height: 1)`; Android
  `SurfaceProducer.setSize` uses the fitted video rect (video aspect via
  `applyBoxFit` / cover scale), not that unit box and not the raw viewport.
- Android display stays on `glimagesink`; a tee→`gldownload`→appsink branch
  fills `frame.c` for `xhvp_player_capture_frame`. Size metadata still comes from
  negotiated **post-orient** caps (`glimagesink` sink after `glvideoflip`, plus
  post-PAUSED query). `glvideoflip` swaps width/height for 90°/270° on GLMemory.
- Video rotation is applied only in the native video-sink bin (`videoflip` on
  desktop/Apple, `glvideoflip` on Android). Dart does not transform the Texture.
  `aspectRatio` follows post-orient size/DAR; `set_rotation` eagerly swaps
  layout metadata when crossing 90/270 so letterboxing updates immediately.
  `open` resets `rotate_degrees` and calls `setVideoRotation(0)`.

## Rate / audio

- Custom playbin `audio-sink` bin: `scaletempo ! audioconvert ! audioresample !
  autoaudiosink` when `scaletempo` is available.
- Speed changes seek with `FLUSH | KEY_UNIT` + `SEEK_TYPE_SET` at the current
  position (not `SEEK_TYPE_NONE` / `INSTANT_RATE_CHANGE`); deferred until
  buffering hits 100%. Desktop/Apple video-sink pins BGRA via capsfilter.

## Dart/native seam testing

- Mock **`PlayerCommandPort`** via **`test/support/fake_player_command_port.dart`**.
- Host smoke: `test/ffi/xhvp_library_test.dart` loads `native/build/host/libxue_hua_video_player.*` when present.
