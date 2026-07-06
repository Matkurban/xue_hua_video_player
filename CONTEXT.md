# xue_hua_video_player — Domain Context

Cross-platform Flutter video player plugin. Decoding via GStreamer (Rust `flutter_rust_bridge` core); rendering via GStreamer platform video sinks bound to Flutter Platform Views.

## Core components

| Term | Meaning |
|------|---------|
| `GstPlayer` | Rust player owning the GStreamer `playbin3` pipeline |
| `XueHuaPlayerController` | Dart controller; exposes signals and playback API |
| `XueHuaVideoView` | Flutter widget embedding a native Platform View for video |
| `playbin3` | GStreamer high-level playback element (URI in, A/V out) |
| `VideoOverlay` | GStreamer interface for rendering into an application-provided native window/surface |

## Platform video sinks (GStreamer recommended)

| Platform | Sink | Flutter integration |
|----------|------|---------------------|
| Android | `glimagesink` | `AndroidView` + `SurfaceView` → `ANativeWindow` |
| iOS | `glimagesink` | `UiKitView` → window handle |
| macOS | `osxvideosink` | `AppKitView` → child `NSView` via VideoOverlay subview |
| Windows | `d3d11videosink` | Platform view → HWND |
| Linux | `glimagesink` | Platform view → X11 window id |

## Rendering model

GStreamer renders directly into the Platform View's native surface via `gst_video_overlay_set_window_handle`. No CPU frame copy, no external Flutter `Texture` widget, no irondash bridge.

## macOS VideoOverlay requirements

- `FlutterPlatformViewFactory` must implement `createArgsCodec()` so `creationParams.playerId` is decoded (otherwise `playerId` stays 0 and overlay never binds).
- Bind the `NSView*` handle before the pipeline reaches `PAUSED` (proactive `set_window_handle`); `set_uri` triggers `READY → PAUSED` immediately.
- Answer `prepare-window-handle` in the pipeline bus sync handler synchronously; if no handle is cached, `osxvideosink` creates a standalone "GStreamer Video Output" window.
- Proactive binding calls `set_window_handle` only — do not call `prepare_window_handle()` from the application side.
- Cache the `NSView*` handle synchronously in `native_window` from Swift C ABI entry points (`player_set_video_overlay_window`, `player_sync_macos_video_layer`).
- Apply the GStreamer overlay bind on the **main thread** via `DispatchQueue.main.async` (`player_apply_macos_overlay_gstreamer`). `osxvideosink` calls `setView:` directly on the main thread; calling from a background thread blocks with `performSelector:waitUntilDone:YES` and deadlocks with Flutter's merged UI/platform thread.
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
