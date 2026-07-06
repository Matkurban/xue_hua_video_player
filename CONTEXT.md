# xue_hua_video_player — Domain Context

Cross-platform Flutter video player plugin. Decoding via GStreamer (Rust `flutter_rust_bridge` core); rendering via GStreamer platform video sinks bound to Flutter Platform Views.

## Core components

| Term | Meaning |
|------|---------|
| `GstPlayer` | Rust player; URI mode uses `playbin3`, asset mode uses `AppSrc` + `decodebin` |
| `XueHuaPlayerController` | Dart controller; exposes signals and playback API |
| `XueHuaVideoView` | Flutter widget embedding a native Platform View for video |
| `playbin3` | GStreamer high-level playback element (URI in, A/V out) |
| `VideoOverlay` | GStreamer interface for rendering into an application-provided native window/surface |

## Platform video sinks (GStreamer recommended)

| Platform | Sink | Flutter integration |
|----------|------|---------------------|
| Android | `glimagesink` | `PlatformViewLink` + `SurfaceView` → `ANativeWindow` |
| iOS | `glimagesink` | `UiKitView` → window handle |
| macOS | `osxvideosink` | `AppKitView` → child `NSView` via VideoOverlay subview |
| Windows | `d3d11videosink` | Platform view → HWND |
| Linux | `glimagesink` | Platform view → X11 window id |

## Rendering model

GStreamer renders directly into the Platform View's native surface via `gst_video_overlay_set_window_handle`. No CPU frame copy, no external Flutter `Texture` widget, no irondash bridge.

## GStreamer runtime (all platforms)

- A dedicated **`xhvp-gst`** thread owns a `MainContext` (`MainContext::new()`, not `default()`) and runs `MainLoop::run()`.
- All pipeline operations (`play`, `pause`, `set_uri`, `set_asset_source`, `seek`, `dispose`) are marshalled onto that thread via `spawn_on_gst_thread_and_wait`.
- Bus events use `bus.add_watch_local` on the Gst thread (no `spawn_bus_thread` polling).
- Position polling uses `timeout_source_new` **attached to the owned Gst `MainContext`** (`gst_main_context()`). Do **not** use `glib::timeout_add_local` — in glib 0.22 it binds to `g_main_context_default()`, which is not the context running `MainLoop::run()` on `xhvp-gst`.
- State transitions call `set_state` then `get_state` with a timeout (`set_state_sync`) so failures surface as explicit errors.
- **Do not** call `set_state_sync` from bus watch callbacks (e.g. `Buffering`) — it blocks the `MainLoop` thread and deadlocks with Android JNI overlay delivery.

## Android VideoOverlay requirements

- `glimagesink` + [`VideoOverlay`](https://gstreamer.freedesktop.org/documentation/rust/stable/latest/docs/gstreamer_video/index.html) bind via `ANativeWindow_fromSurface` from `SurfaceView` callbacks.
- **Never** call `spawn_on_gst_thread_and_wait` from Android JNI / main thread (`surfaceCreated` / `surfaceChanged`). Cache the native window handle on the JNI thread, then apply overlay + `set_render_rectangle` + `expose` via `spawn_on_gst_thread` (fire-and-forget).
- If no overlay handle is cached when `set_uri` / `set_asset_source` runs, defer `PAUSED` preroll until the first surface bind (`maybe_preroll_after_overlay_bind`).
- Answer `prepare-window-handle` in the pipeline bus sync handler; proactive `set_window_handle` before preroll is preferred.
- Flutter Android uses `PlatformViewLink` + `initSurfaceAndroidView` (hybrid composition) — not legacy virtual-display `AndroidView` — so `SurfaceView` gets a reliable surface.
- `GStreamerInitProvider` loads `gstreamer_android` then `xue_hua_video_player` before Dart FRB `dlopen`.

## Android native library load order

- `GStreamerInitProvider` and `XueHuaVideoPlayerPlugin` call `System.loadLibrary("xue_hua_video_player")` at process/plugin startup — **before** Dart `RustLib.init()` (`DynamicLibrary.open`).
- If FRB `dlopen` runs first, `XueHuaVideoPlatformView.nativeOnSurfaceCreated` fails with `UnsatisfiedLinkError` even though symbols exist in the `.so`.
- `gstreamer_android` is still loaded first via `GStreamerInitProvider` so `JNI_OnLoad` + `GStreamer.init(context)` run for MediaCodec decoders.

## Asset playback (AppSrc)

- Flutter assets are opened in Rust (`asset_resolver`): `flutter_assets/{key}` on desktop, `AssetManager` on Android.
- Bytes are pushed through `gstreamer-app` `AppSrc` into `decodebin`, sharing the same `VideoOverlay` video sink as URI mode.
- Dart calls `playerSetAssetSource(assetKey)` — no temp-file copy under `xhvp_assets`.

## macOS VideoOverlay requirements

- `FlutterPlatformViewFactory` must implement `createArgsCodec()` so `creationParams.playerId` is decoded (otherwise `playerId` stays 0 and overlay never binds).
- Bind the `NSView*` handle before the pipeline reaches `PAUSED` (proactive `set_window_handle`); `set_uri` triggers `READY → PAUSED` immediately.
- Answer `prepare-window-handle` in the pipeline bus sync handler synchronously; if no handle is cached, `osxvideosink` creates a standalone "GStreamer Video Output" window.
- Proactive binding calls `set_window_handle` only — do not call `prepare_window_handle()` from the application side.
- Cache the `NSView*` handle synchronously in `native_window` from Swift C ABI entry points (`player_set_video_overlay_window`, `player_sync_macos_video_layer`).
- Apply the GStreamer overlay bind on the **main thread** via `DispatchQueue.main.async` (`player_apply_macos_overlay_gstreamer`). `osxvideosink` calls `setView:` directly on the main thread; calling from a background thread blocks with `performSelector:waitUntilDone:YES` and deadlocks with Flutter's merged UI/platform thread.
- `player_apply_macos_overlay_gstreamer` must call `set_window_handle` **directly on the main thread** using a cached `overlay_sink` clone — it must **not** use `spawn_on_gst_thread_and_wait` (pipeline ops stay on xhvp-gst; VideoOverlay apply is the exception).
- `play()` / `set_uri()` verify the overlay handle is cached; GStreamer bind runs on the main thread (Swift `DispatchQueue.main.async`) or via the bus `prepare-window-handle` sync handler. Do not call `apply_macos_overlay_gstreamer` from the FRB thread pool.
- Do **not** use a dedicated overlay background thread combined with `drain_overlay_queue()` — that creates a circular wait with `osxvideosink`'s main-thread dispatch.
- HTTPS requires the GIO OpenSSL TLS backend (`register_gio_tls_backend()` after `gst::init()`); without it `souphttpsrc` delivers zero bytes.
- Use a child `NSView` (`wantsLayer = false`) as the VideoOverlay target inside the Flutter platform view.

## Deprecated / removed

- `irondash_texture`, `irondash_engine_context` — replaced by Platform View + VideoOverlay
- `appsink` RGBA frame buffer path — replaced by platform sinks
- `textureId` on controller — replaced by Platform View lifecycle tied to `player_id`

## References

- [GStreamer gst-docs](https://github.com/GStreamer/gst-docs)
- [Android tutorial 3: Video](https://gstreamer.freedesktop.org/documentation/tutorials/android/video.html)
