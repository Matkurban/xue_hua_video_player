# xue_hua_video_player — Domain Context

Cross-platform Flutter video player plugin. Decoding via GStreamer (Rust `flutter_rust_bridge` core); rendering via GStreamer platform video sinks bound to Flutter Platform Views.

## Core components

| Term | Meaning |
|------|---------|
| `PlaybackEngine` | Rust player (formerly `GstPlayer`); URI mode uses `playbin3`, asset mode uses `AppSrc` + `decodebin` |
| `PipelineShell` | Shared sinks, bus handlers, and overlay sync wiring for both source adapters |
| `MediaSource` | Unified load descriptor: `Uri` or `FlutterAsset`, resolved in `media/` |
| `GstPlayer` | Type alias for `PlaybackEngine` (backward compatible) |
| `XueHuaPlayerController` | Dart controller; exposes signals and playback API |
| `XueHuaVideoView` | Flutter widget embedding a native Platform View for video |
| `playbin3` | GStreamer high-level playback element (URI in, A/V out) |
| `VideoOverlay` | GStreamer interface for rendering into an application-provided native window/surface |

## Platform video sinks (GStreamer recommended)

| Platform | Sink | Flutter integration |
|----------|------|---------------------|
| Android | `glimagesink` | `PlatformViewLink` + `SurfaceView` → `ANativeWindow` |
| iOS | `avsamplebufferlayersink` | `UiKitView` → host `UIView` + sink `CALayer` sublayer |
| macOS | `osxvideosink` | `AppKitView` → child `NSView` via VideoOverlay subview |
| Windows | `d3d11videosink` | Platform view → HWND |
| Linux | `glimagesink` | Platform view → X11 window id |

## Rendering model

GStreamer renders directly into the Platform View's native surface via `gst_video_overlay_set_window_handle`. No CPU frame copy, no external Flutter `Texture` widget, no irondash bridge.

## GStreamer runtime (all platforms)

- A dedicated **`xhvp-gst`** thread owns a `MainContext` (`MainContext::new()`, not `default()`) and runs `MainLoop::run()`.
- All pipeline operations (`play`, `pause`, `load`, `seek`, `dispose`) are marshalled onto that thread via `spawn_on_gst_thread_and_wait`.
- Bus events use `bus.add_watch_local` on the Gst thread (no `spawn_bus_thread` polling).
- Position polling uses `timeout_source_new` **attached to the owned Gst `MainContext`** (`gst_main_context()`). Do **not** use `glib::timeout_add_local` — in glib 0.22 it binds to `g_main_context_default()`, which is not the context running `MainLoop::run()` on `xhvp-gst`.
- State transitions call `set_state` then `get_state` with a timeout (`set_state_sync`) so failures surface as explicit errors.
- **Do not** call `set_state_sync` from bus watch callbacks (e.g. `Buffering`) — it blocks the `MainLoop` thread and deadlocks with Android JNI overlay delivery.

## Android VideoOverlay requirements

- `glimagesink` + [`VideoOverlay`](https://gstreamer.freedesktop.org/documentation/rust/stable/latest/docs/gstreamer_video/index.html) bind via `ANativeWindow_fromSurface` from `SurfaceView` callbacks.
- **Never** call `spawn_on_gst_thread_and_wait` from Android JNI / main thread (`surfaceCreated` / `surfaceChanged`). Cache the native window handle on the JNI thread, then apply overlay + `set_render_rectangle` + `expose` via `spawn_on_gst_thread` (fire-and-forget).
- If no overlay handle is cached when `load` runs, defer `PAUSED` preroll until the first surface bind (`maybe_preroll_after_overlay_bind`).
- After `PipelineShell` rebuild (URI ↔ asset switch), `mark_shell_rebuilt()` clears `overlay_bound`; `rebind_cached_overlay()` on the same Gst-thread stack must re-apply VideoOverlay to the new `video_sink` and set `overlay_bound` before preroll/play.
- Answer `prepare-window-handle` in the pipeline bus sync handler; proactive `set_window_handle` before preroll is preferred.
- Flutter Android uses `PlatformViewLink` + `initSurfaceAndroidView` (hybrid composition) — not legacy virtual-display `AndroidView` — so `SurfaceView` gets a reliable surface.
- `GStreamerInitProvider` loads `gstreamer_android` then `xue_hua_video_player` before Dart FRB `dlopen`.

## iOS video sink requirements

- iOS uses **`avsamplebufferlayersink`** (applemedia plugin): GStreamer exposes a `CALayer` via the sink `layer` property; Swift attaches it as a sublayer of the Flutter host `UIView` on the **main thread** (`xhvp_ios_attach_layer_to_host` / `player_apply_ios_overlay_gstreamer`).
- **Do not** use `glimagesink` / `EaglUIView` / `VideoOverlay::set_window_handle` on iOS — GL draw callbacks access UIKit off the main thread during network preroll.
- Swift caches the host `UIView*` via `player_notify_ios_overlay` (**cache only** — does not trigger Gst attach), then `DispatchQueue.main.async { player_apply_ios_overlay_gstreamer }` after layout (same pattern as macOS `player_apply_macos_overlay_gstreamer`).
- Rust attaches the sink layer and prerolls on **`xhvp-gst`** inside `apply_ios_overlay_gstreamer`; do not call `spawn_on_gst_thread_and_wait` from UIKit layout callbacks.
- **Do not** `dispatch_sync` to the main thread from code running inside `spawn_on_gst_thread_and_wait` when the caller is the Flutter UI thread (deadlock). First-bind CALayer attach from **xhvp-gst** uses `xhvp_ios_attach_layer_to_host_sync` (`dispatch_sync` main queue); never sync from the Flutter UI thread.
- **iOS Tutorial 4 `setUri`:** only `READY` + set `uri` on iOS — no switch-side `PAUSED` preroll. Preroll + attach live in `IosOverlaySession` / `attach_ios_video_layer_with_completion`.
- **iOS Tutorial 4 `setUri` preroll gate:** `has_cached_handle()` only — not `overlay_bound`. If no host view is cached when `load` runs, defer attach until `player_notify_ios_overlay` caches the `UIView*`.
- **iOS Tutorial 4 `target_state`:** `PLAYING` requires **verified** `overlay_bound` (CALayer attached to host with non-zero bounds and confirmed in hierarchy), not just a cached host view. **`desired_playing` maps to `target_state`.** All iOS `PLAYING` / `PAUSED` transitions (play, buffering, clock-lost, attach completion) go through **`IosOverlaySession::apply_target_state`** scheduled on the Gst `MainContext` idle — bus/engine must **never** call `set_state_sync` on iOS.
- **Media reload:** `VideoSurface::mark_media_changed()` on every `PlaybackEngine::load` clears `overlay_bound` (URI→URI reload, not only shell kind change).
- **iOS Tutorial 4 `check_media_size` timing:** attach sink `CALayer` on bus `READY → PAUSED` and via `IosOverlaySession` after load/play — not at `READY` like `glimagesink` + VideoOverlay.
- **Verified CALayer attach:** `xhvp_ios_attach_layer_to_host_sync` returns `bool` — host bounds must be non-zero and sublayer must appear in hierarchy. `read_sink_layer` `CFRetain`s; shim `CFRelease`s only on verified attach; Rust `release_sink_layer` on defer/failure. **No PAUSED preroll** until `xhvp_ios_host_view_has_bounds` is true; failed post-preroll attach rolls pipeline back to `READY`. First attach uses async main-thread CALayer bind (resize re-attach may use sync). **First attach with pending media always prerolls to PAUSED before reading `layer`** — do not attach when the sink `layer` property is readable at `READY` but decode has not prerolled.
- **Host-change reset:** `reset_for_host_change` runs only when the cached host pointer actually changes (not on first bind). It is skipped while `attach_in_flight` is set.
- **`IosOverlaySession`** (`playback/ios_overlay.rs`): single seam for iOS overlay attach phase — `request_attach` dedupes via `attach_in_flight`, `finish_attach` sets `overlay_bound` only after verified attach, `schedule_apply` / `schedule_attach` coalesce idle work, and `apply_target_state` implements Tutorial 4 `target_state` + Tutorial 12 buffering (`buffering_active`). Bus callbacks set flags only — zero `set_state_sync` / inline attach on iOS. **`overlay_generation`** invalidates queued idle/spawn work on `load`, shell rebuild, and `PlaybackEngine::Drop`; paired with `running` to match bus-watch teardown semantics.
- **Layout retry:** `player_notify_ios_overlay` caches handle/dimensions only; Swift `scheduleOverlayApply` triggers Gst attach when bounds are non-zero (not only when `!overlay_bound`); zero bounds defer attach until layout.
- **xhvp-gst threading:** `spawn_on_gst_thread_and_wait` / `run_on_gst_thread` runs inline when `MainContext::is_owner()` — never nest invoke+recv on the same Gst thread. `gst::init()` runs once in `gst_runtime_thread_main`; `ensure_gst_init` only registers iOS plugins/TLS on xhvp-gst.
- iOS bus `prepare-window-handle`: **Pass** (ignored) — `avsamplebufferlayersink` uses `IosOverlaySession` sync CALayer attach, not VideoOverlay `dispatch_sync` bind.
- iOS GStreamer plugins are statically linked in `GStreamer.framework`; register via `register_ios_static_plugins()` including `gst_plugin_applemedia_register()`. HTTPS uses OpenSSL + DarwinSSL GIO TLS backends (`register_gio_tls_backend()`).

## Android native library load order

- `GStreamerInitProvider` and `XueHuaVideoPlayerPlugin` call `System.loadLibrary("xue_hua_video_player")` at process/plugin startup — **before** Dart `RustLib.init()` (`DynamicLibrary.open`).
- If FRB `dlopen` runs first, `XueHuaVideoPlatformView.nativeOnSurfaceCreated` fails with `UnsatisfiedLinkError` even though symbols exist in the `.so`.
- `gstreamer_android` is still loaded first via `GStreamerInitProvider` so `JNI_OnLoad` + `GStreamer.init(context)` run for MediaCodec decoders.

## Asset playback (AppSrc)

- Flutter assets are resolved in Rust (`media/resolver`): `flutter_assets/{key}` on desktop, `AssetManager` on Android.
- **Darwin bundle roots:** iOS executable is `Runner.app/Runner` → bundle root is `exe.parent()`; macOS is `Contents/MacOS/Runner` → bundle root is `exe.parent().parent()` (`Contents`). Framework assets live at `Frameworks/App.framework/flutter_assets/{key}`.
- **iOS init:** `XueHuaVideoPlayerPlugin.register` calls `xhvp_set_flutter_assets_dir` with `Bundle.main` `App.framework/flutter_assets` so resolution works even when `current_exe()` differs under the debugger.
- When a bundle path resolves on iOS/macOS/desktop, `MediaSource::FlutterAsset` becomes `file://` + playbin3 (seekable). AppSrc fallback is used on Windows/Linux when path resolution fails; Darwin returns the error directly.
- Bytes are pushed through `gstreamer-app` `AppSrc` into `decodebin`, sharing the same `VideoOverlay` video sink as URI mode.
- AppSrc EOS replay reloads the asset shell (`teardown` + fresh `decodebin` pipeline) via `replay_asset_shell`; in-place rewind/state cycles break `pad_added` topology.
- Prefer `playerLoadSource` / `PlaybackEngine::load(MediaSource)` — legacy `playerSetAssetSource` delegates to the same path.
- Local files use `MediaSource::Uri` with a `file://` URI (Dart: `VideoSource.file`).

## Source switching

- `playback/switch.rs` exposes `switch_shell(resolved, ctx)` — the single Gst-thread entry for URI ↔ asset transitions.
- `VideoSurface` (`playback/surface.rs`) owns cached overlay handles and platform bind scheduling (Android/iOS Gst-thread defer, macOS main-thread apply).
- `PipelineCapabilities` (`playback/capabilities.rs`) types playbin-only features (seek, tracks, orientation); AppSrc pipelines report reduced capability.
- Playbin track lists are cached on bus `StreamCollection` / `StreamsSelected` and enriched from `GstStream::tags()` — playbin3 does not expose legacy `n-audio` properties.

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
