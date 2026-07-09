# xue_hua_video_player — Domain Context

Cross-platform Flutter video player plugin. Decoding via GStreamer (Rust `flutter_rust_bridge` core); rendering via Flutter external **`Texture`** widgets backed by a custom native bridge (no third-party texture plugins).

## Core components

| Term | Meaning |
|------|---------|
| `PlaybackEngine` | Rust player (formerly `GstPlayer`); URI mode uses `playbin3`, asset mode uses `AppSrc` + `decodebin` |
| `PipelineShell` | Deep module (`playback/shell.rs`): private `pipeline` / `video_sink` / lifecycle fields; public method API (`set_state_sync`, `set_uri`, `snapshot()`, `apply_aspect_ratio`, …); `SourceKind` + `source_kind()` / `is_uri()` / `asset_key()` |
| `MediaSource` | Unified load descriptor: `Uri` or `FlutterAsset`, resolved in `media/` |
| `GstPlayer` | Type alias for `PlaybackEngine` (backward compatible) |
| `XueHuaPlayerController` | Dart **public facade** (thin): delegates to **`PlaybackSession`**; implements [PlaybackControlsModel] + presentation extras (`playerId`, `tracks`, `setAspectRatioMode`, …); **does not** hold `PlayerCommandPort` directly |
| `PlaybackSession` | Dart **deep orchestration module** (`lib/src/player/playback_session.dart`): owns signals, event dispatch, `open()` lifecycle, `_guard`, optimistic transport commands, `tracksChanged` refresh; inward seam **`PlayerCommandPort`**; outward implements **`PlaybackControlsModel`** transport + exposes full readonly signals |
| `PlaybackControlsModel` | Narrow controls seam: readonly transport signals + commands (play/seek/mute/…); widgets / **`ScrubController`** depend on this — **not** on `XueHuaPlayerController` concrete type (workstream C) |
| `ScrubController` | Drag/seek settle logic for built-in sliders; depends on [PlaybackControlsModel] only — **unchanged in workstream B** |
| `TransportCommand` | **Not a separate type** — private `_preview*` / `_guard` helpers on **`PlaybackSession`** (seek/volume/mute/speed/looping optimistic transport) |
| `PlayerCommandPort` | Dart/Rust seam adapter: create/dispose, event stream, FRB playback commands — **implementation detail of `PlaybackSession`** (B3); prod + **`FakePlayerCommandPort`** adapters retained |
| `PlayerStateStore` | **Removed** (workstream B1) — logic lives in **`PlaybackSession`** |
| `MediaSourceResolver` | Dart: `VideoSource` → `MediaSourceDto` before crossing the Rust seam |
| `PlaybackPresentationModel` | Narrow presentation seam: `playerId`, `aspectRatio`, buffering/loading signals, `setAspectRatioMode` — widgets bind this instead of `XueHuaPlayerController` concrete type |
| `PlaybackPresentation` | Deep presentation widget (`lib/src/presentation/`): `VideoSurfaceHandle` routing + aspect layout + buffering chrome + **`AspectRatioMode`** pipeline sync |
| `XueHuaVideoView` | Thin layout shell: `PlaybackPresentation` + optional `VideoControls`; background color + controls style only |
| `playbin3` | GStreamer high-level playback element (URI in, A/V out) |
| `VideoOverlay` | GStreamer interface for rendering into an application-provided native window/surface |
| `VideoSurface` | Thin delegate (`playback/surface.rs`): `stored` handle + platform `OverlaySession`; FRB/engine entry points delegate to session |
| `OverlaySession` | Unified Rust trait (`playback/overlay/overlay_session.rs`): load preroll + `cache_notify` + `apply_gstreamer` + lifecycle; **`apply_load_preroll`** calls **`gate_ready_for_load(surface.overlay_ready_for_preroll())`** internally |
| `resume_playing` | Unified play/EOS resume (`playback/play_resume.rs`): `resume_playing(shell, replay, swap, surface, overlay_ready)` — engine, Android/iOS bind preroll, macOS/desktop `pipeline_play` |
| `maybe_resume_after_overlay_bind` | Bind-complete resume (`playback/play_resume.rs`): if `desired_playing`, Ready→Paused preroll when needed then `resume_playing(..., true)` — macOS `apply_gstreamer`, Win/Linux `set_window_handle_on_gst` |
| `overlay_ready_for_play` | Platform gate for `resume_playing`: texture platforms (iOS/macOS/Win/Linux appsink) always ready; Android uses `surface.is_overlay_bound_on_gst()` |
| `VideoOverlayBackend` | Desktop-only stored-handle trait for tests; mobile/desktop sessions implement **`OverlaySession`** |
| `AndroidOverlaySession` | Android overlay attach phase (`playback/overlay/platform/android/session.rs`): JNI `notify_surface_with_shell`, `schedule_apply_after_bind`, `overlay_generation`, bind-path **`PrerollExecutor`** |
| `IosOverlaySession` | iOS CALayer attach + idle `apply_target_state` (`playback/overlay/platform/ios/session.rs`); owns `ios_layer_bus_slot`, dimensions, `apply_gstreamer` (Swift layout apply) |
| `MacosOverlaySession` | macOS window overlay (`playback/overlay/platform/window/session.rs`): `overlay_bound` after main-thread `apply_gstreamer`; schedules **`maybe_resume_after_overlay_bind`** on `xhvp-gst` |
| `DesktopOverlaySession` | Win/Linux window overlay: `overlay_bound` after `set_window_handle`; bind path calls **`maybe_resume_after_overlay_bind`** |
| `IosLayerBackend` | Thin iOS bus-facing adapter; holds **`Arc<PlaybackGstContext>`**; delegates to **`IosOverlaySession`**; **`OverlayPlayIntent`** via **`ctx.overlay_intent()`** |
| `PlaybackGstContext` | Engine-owned Gst bundle (`shell`, `surface`, `replay`, swap sources) in `playback/gst_context.rs`; **`clone_for_async()`** snapshots orientation/aspect; **`overlay_intent()`** for mobile overlay |
| `PrerollExecutor` | Shared 4-step `decide_preroll_action` loop (`playback/overlay/preroll/executor.rs`); GStreamer I/O via injected **`PrerollEffects`** adapter per platform (**bind path only**) |
| `PrerollEffects` | Platform side-effect adapter for `PrerollExecutor` (`pause_preroll`, `resume_playing`, overlay refresh on Android) — mockable in unit tests |
| `LoadPrerollPolicy` | **Removed** — merged into **`OverlaySession`** (`gate_ready_for_load` + `apply_load_preroll`); `switch.rs` calls `surface.overlay_session().apply_load_preroll(shell, surface, defer_log)` |
| `PipelineSwapConfig` | Pipeline-only swap metadata (`metadata`, `track_cache`, `orientation`, `aspect`, `emitter`, `looping`) — replaces fat **`ShellTransition`**; does **not** hold replay atomics or `VideoSurface` |
| `GstTaskScheduler` | Injectable seam for fire-and-forget Gst work (`spawn_on_gst_thread` in prod, sync queue in tests); used by overlay sessions for apply scheduling |
| `BusMessage` | Parsed Gst bus input (`playback/bus/parse.rs`) — gst-free enum for pure reduction |
| `BusSnapshot` | Read-only reducer inputs: `desired_playing`, `looping`, `is_playbin`, `overlay_bound` |
| `reduce_bus_message` | Pure bus reducer (`playback/bus/reducer.rs`): `(BusMessage, BusSnapshot) → events + BusSideEffect + BusReplayPatch` |
| `BusSideEffect` | Semantic imperative intents (`PausePipelineForBuffering`, `IosScheduleApply`, `TrackCacheSyncFromCollection`, …); platform `#[cfg]` only in executor |
| `BusReplayPatch` | Reducer output for replay atomics (`at_eos`, `desired_playing`); applied before emit/effects |
| `attach_gst_bus_handlers` | Bus watch entry (`playback/bus/mod.rs`): parse → reduce → patch → emit → `apply_bus_side_effects`; 200ms position poll unchanged |

## Rust `src/` layout (reorganized)

| Path | Role |
|------|------|
| `api/` | FRB seam: `player.rs` registry, `types.rs` DTOs (`player_events` alias for generated code) |
| `gst/` | Process GStreamer bootstrap: `init`, `runtime` (`xhvp-gst`), `android`, `android_bootstrap` (AndroidNativeRuntimeBootstrap), `tls`, `env`, `ios_plugins` |
| `AndroidNativeRuntimeBootstrap` | Deep Android process-start module (`gst/android_bootstrap.rs`): `warmup()` / `ensure_ready_for_network_preroll()`; owns FRB handler touch, `xhvp-gst`, GstGL display sync warmup, reqwest factory readiness. Java seam: `NativeRuntimeWarmup` → JNI → this module |
| `platform/` | Native FFI: `jni`, `android`, `ios/` (`mod.rs` UIKit bridge, `layer.rs` CALayer) |
| `playback/gst/` | GStreamer video primitives: `sink.rs` (VideoOverlay), `metadata.rs`, `orientation.rs` — **not** overlay lifecycle |
| `playback/overlay/` | `overlay_session.rs`, `preroll/{gate,executor}`, `platform/{android,ios,window}/` |
| `playback/bus/` | `parse`, `reducer`, `effects`, `mod` (bus watch entry) |

## Platform video sinks (current)

| Platform | Sink | Flutter integration |
|----------|------|---------------------|
| Android | `glimagesink` | `TextureRegistry.SurfaceProducer` → `Surface` → `ANativeWindow` (VideoOverlay) |
| iOS / macOS | `appsink` (BGRA) | `FlutterTexture` + IOSurface-backed `CVPixelBuffer` |
| Windows / Linux | `appsink` (BGRA) | `PixelBufferTexture` / `FlPixelBufferTexture` (RGBA upload) |

## Rendering model

- **Apple + desktop:** GStreamer terminates in `appsink`; Rust `FrameSink` double-buffers BGRA frames and exposes a C-ABI (`xhvp_texture_*`) for native texture plugins to pull pixels and call `textureFrameAvailable` / `MarkTextureFrameAvailable`.
- **Android:** GStreamer `glimagesink` renders with OpenGL into the `SurfaceProducer` surface (zero-copy); JNI `AndroidSurfaceBridge` forwards surface lifecycle to `AndroidOverlaySession`.
- Dart embeds video with the Flutter `Texture` widget (`TextureVideoSurface`); native plugins register textures via MethodChannel `xue_hua_video_player/texture` (`createTexture` / `disposeTexture`).

## GStreamer runtime (all platforms)

- A dedicated **`xhvp-gst`** thread owns a `MainContext` (`MainContext::new()`, not `default()`) and runs `MainLoop::run()`.
- All pipeline operations (`play`, `pause`, `load`, `seek`, `dispose`) are marshalled onto that thread via `spawn_on_gst_thread_and_wait`.
- Bus events use `bus.add_watch_local` on the Gst thread (no `spawn_bus_thread` polling).
- Bus watch pipeline: **`parse_bus_message`** → **`reduce_bus_message`** → **`BusReplayPatch`** → emit **`PlayerEvent`** → **`apply_bus_side_effects`** (`playback/bus/`). Reducer is pure (table-tested); GStreamer I/O and platform `#[cfg]` live in parse/executor only.
- Position polling uses `timeout_source_new` **attached to the owned Gst `MainContext`** (`gst_main_context()`). Do **not** use `glib::timeout_add_local` — in glib 0.22 it binds to `g_main_context_default()`, which is not the context running `MainLoop::run()` on `xhvp-gst`.
- State transitions call `set_state` then `get_state` with a timeout (`set_state_sync`) so failures surface as explicit errors.
- **Do not** call `set_state_sync` from bus watch callbacks (e.g. `Buffering`, `ClockLost`) — use async `pipeline.set_state()` only; blocking `get_state` deadlocks the MainLoop (Android JNI overlay, macOS `osxvideosink`).
- **Android:** Gradle downloads the official GStreamer Android SDK (if missing) and runs ndk-build to produce `libgstreamer_android.so` per ABI before the Rust link step. Cache default: `~/Library/Caches/xue_hua_video_player/gstreamer/android/<GST_VER>/`. Override with `GSTREAMER_ROOT_ANDROID` / `GST_VER`. See `android/scripts/` and `android/build.gradle`.

## Android VideoOverlay requirements

- `glimagesink` + [`VideoOverlay`](https://gstreamer.freedesktop.org/documentation/rust/stable/latest/docs/gstreamer_video/index.html) bind via `ANativeWindow_fromSurface` from `SurfaceView` callbacks.
- **HW decode (`amcvideodec`)** emits External OES GL textures; URI/AppSrc video branches use `glupload → glcolorconvert → gltransformation → glimagesink` (`playback/gst/sink.rs`). Do not insert `gldownload`, `videoflip`, or `videoconvert` on this path — `gldownload` cannot map External OES and causes black video with audio.
- **Never** call `spawn_on_gst_thread_and_wait` from Android JNI / main thread (`surfaceCreated` / `surfaceChanged`). Cache the native window handle on the JNI thread, then apply overlay + `set_render_rectangle` + `expose` via `spawn_on_gst_thread` (fire-and-forget).
- If no overlay handle is cached when `load` runs, defer `PAUSED` preroll until the first surface bind (`maybe_preroll_after_overlay_bind`).
- After `PipelineShell` rebuild (URI ↔ asset switch), `mark_shell_rebuilt()` clears `overlay_bound`; `rebind_cached_overlay()` on the same Gst-thread stack must re-apply VideoOverlay to the new `video_sink` and set `overlay_bound` before preroll/play.
- **`PipelineShell`:** all GStreamer field access goes through shell methods; `PipelineSnapshot::from_shell` removed — use **`shell.snapshot()`**; `state.rs` deleted (`set_state_sync` → **`PipelineShell::set_state_sync`**; bus uses **`set_element_state_sync`** for cloned pipelines).
- Answer `prepare-window-handle` in the pipeline bus sync handler; proactive `set_window_handle` before preroll is preferred.
- Flutter Android uses `TextureRegistry.createSurfaceProducer()`; `XueHuaVideoTexture` implements `SurfaceProducer.Callback` and forwards surfaces through **`AndroidSurfaceBridge`** JNI to Rust (never `spawn_on_gst_thread_and_wait` from JNI).
- `GStreamerInitProvider` loads `gstreamer_android` then `xue_hua_video_player` before Dart FRB `dlopen`.
- **`AndroidOverlaySession`** (`playback/overlay/platform/android/session.rs`): single seam for Android overlay bind phase — mirrors `IosOverlaySession` shape. **`VideoSurface::notify_android_surface`** keeps one FRB/JNI entry; internally **cache** handle/dimensions on the JNI thread, then **`schedule_apply_after_bind`** on **`xhvp-gst`** via **`GstTaskScheduler`** (never `spawn_on_gst_thread_and_wait` from JNI). **`overlay_generation`** invalidates queued apply on `load`, shell rebuild, and surface destroy. **`PrerollExecutor`** + Android **`PrerollEffects`** run bind-path preroll (bind ops in `platform/android/ops.rs`).

## iOS / macOS texture requirements

- Pipeline video sink is **`appsink`** (BGRA caps), not `avsamplebufferlayersink` / `osxvideosink`.
- Native `XueHuaVideoTexture` (`FlutterTexture`) pulls frames via `xhvp_texture_*`, wraps them in IOSurface-backed `CVPixelBuffer`, and notifies `textureFrameAvailable`.
- `load` / `play` do not wait for Platform View layout — texture platforms are always overlay-ready (`overlay_ready_for_play` returns true).
- iOS `xhvp_set_flutter_assets_dir` is still set from `XueHuaVideoPlayerPlugin.register` for bundle asset resolution.

## iOS video sink requirements (removed — historical)

<details>
<summary>Legacy Platform View + avsamplebufferlayersink notes (pre-1.4)</summary>
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
- **`IosOverlaySession`** (`playback/overlay/platform/ios/session.rs`): single seam for iOS overlay attach phase — `request_attach` dedupes via `attach_in_flight`, `finish_attach` sets `overlay_bound` only after verified attach, `schedule_apply` / `schedule_attach` coalesce idle work, and `apply_target_state` implements Tutorial 4 `target_state` + Tutorial 12 buffering (`buffering_active`) using shared **`PrerollExecutor`** + iOS **`PrerollEffects`**. Bus callbacks set flags only via **`BusSideEffect`** (`IosSetBufferingActive`, `IosScheduleApply`, …) — zero `set_state_sync` / inline attach on iOS. **`overlay_generation`** invalidates queued idle/spawn work on `load`, shell rebuild, and `PlaybackEngine::Drop`; paired with `running` to match bus-watch teardown semantics. CALayer FFI lives in **`platform/ios/layer.rs`**; UIKit bridge in **`platform/ios/mod.rs`**.
- **Layout retry:** `player_notify_ios_overlay` caches handle/dimensions only; Swift `scheduleOverlayApply` triggers Gst attach when bounds are non-zero (not only when `!overlay_bound`); zero bounds defer attach until layout.
- **xhvp-gst threading:** `spawn_on_gst_thread_and_wait` / `run_on_gst_thread` runs inline when `MainContext::is_owner()` — never nest invoke+recv on the same Gst thread. `gst::init()` runs once in `gst/runtime.rs` thread main; `ensure_gst_init` (`gst/init.rs`) only registers iOS plugins/TLS on xhvp-gst. Process bootstrap lives under **`rust/src/gst/`** (init, runtime, android, tls).
- iOS bus `prepare-window-handle`: **Pass** (ignored) — `avsamplebufferlayersink` uses `IosOverlaySession` sync CALayer attach, not VideoOverlay `dispatch_sync` bind.
- iOS GStreamer plugins are statically linked in `GStreamer.framework`; register via `register_ios_static_plugins()` including `gst_plugin_applemedia_register()`. HTTPS uses OpenSSL + DarwinSSL GIO TLS backends (`register_gio_tls_backend()`).

</details>

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

## Dart rendering surface

- **`lib/src/presentation/`** (partial export): **`PlaybackPresentation`** deep widget + **`PlaybackPresentationModel`** seam; owns surface embed, aspect sync, loading chrome.
- **`lib/src/surface/`** (not exported): **`VideoSurfaceHandle`** routing + **`TextureVideoSurface`** (`Texture` widget + native texture MethodChannel).
- Full UI prefers **`XueHuaVideoView`**; surface-only with aspect sync uses package-private **`PlaybackPresentation`** (same stack as the view without controls).
- **`PlaybackPresentation.aspectRatioMode`** syncs **`AspectRatioMode`** to the Rust pipeline where the sink supports it; letterbox/crop/stretch is applied in GStreamer, not Flutter `BoxFit`.

## Dart playback presentation (workstream C)

- **Before:** surface routing, aspect sync, loading overlay lived in **`XueHuaVideoView`**; presentation logic bound to controller concrete type.
- **After (C1 landed):** **`PlaybackPresentation`** owns embed + sync + chrome; **`XueHuaVideoView`** is a thin shell (~40 LOC).
- **`PlaybackSession`** / **`XueHuaPlayerController`** implement **`PlaybackPresentationModel`** alongside **`PlaybackControlsModel`**.
- Custom layouts: **`XueHuaVideoView`** (`showControls: false`) or package-private **`PlaybackPresentation`** for aspect sync without the built-in bar.

## Dart playback orchestration (workstream B)

- **Before:** shallow **`XueHuaPlayerController`** + **`PlayerStateStore`** + **`PlayerCommandPort`** — command orchestration, optimistic UI, `open()` lifecycle, and `tracksChanged` refresh split across three modules; seek spans controller → store → port → **`ScrubController`**.
- **After (B1–B3 landed):** **`PlaybackSession`** owns signals, reducer, transport, and port; **`XueHuaPlayerController`** is a thin delegate (~100 LOC).
- **B1:** event reducer + `initialize()` subscription + `open()` (`resetForOpen` → `loadSource` → capabilities → tracks) + `_guard` + **`tracksChanged` → `_refreshTracks()` inside session** (remove dead reducer arm + controller special-case).
- **B2:** transport helpers as private `_preview*` methods on session — no separate `TransportCommand` file unless session exceeds ~400 LOC.
- **B3:** only **`PlaybackSession`** talks to **`PlayerCommandPort`**; controller ctor injects `PlaybackSession?` / port for tests — drop direct `PlayerStateStore?` injection.
- **B4 (deferred):** **`MediaSourceResolver`** stays honest shallow injectable; deepen only if `open()` gains Dart-side policy (validation/cache).
- **Workstream C dependency:** satisfied — presentation binds **`PlaybackPresentationModel`**; controls bind **`PlaybackControlsModel`**.

## Testing

- **Dart orchestration**: primary test surface is **`PlaybackSession`** with **`FakePlayerCommandPort`**; migrate existing controller tests to `test/player/playback_session_test.dart`; keep thin controller smoke tests.
- **Dart/Rust seam**: mock **`PlayerCommandPort`** via **`test/support/fake_player_command_port.dart`**; do **not** use `RustLib.initMock` for session/controller tests.
- **Event boilerplate**: **`test/support/player_event_fixtures.dart`** builds `PlayerEvent` values for reducer/session tests.
- **Controls seam**: **`FakePlaybackControlsModel`** for widget tests under `test/controls/`.
- **Presentation seam**: **`FakePlaybackPresentationModel`** + `test/presentation/playback_presentation_test.dart` (aspect sync, loading chrome — no PlatformView embed).
- **Rust pure logic**: `overlay/preroll/gate`, `overlay/preroll/executor` (+ `RecordingPrerollEffects`), **`OverlaySession`** + **`play_resume`** (`plan_resume_action` table tests, `FakeOverlaySession`), overlay session scheduling (+ injectable `GstTaskScheduler`), `replay`, overlay mock backends, resolver/shell/tracks — run with `cd rust && cargo test`.
- **Out of scope for unit tests**: real GStreamer pipelines, PlatformView native embed, device/integration playback (use manual QA or future `integration_test` work).

## Source switching

- `playback/switch.rs` exposes `switch_shell(resolved, …)` — the single Gst-thread entry for URI ↔ asset transitions. **Target API (A4):** `switch_shell(shell, resolved, swap: &PipelineSwapConfig, replay: &PlayReplayContext, surface: &VideoSurface)` — no separate `ios_layer_bus_slot` parameter (slot lives on `VideoSurface`).
- **`PipelineSwapConfig`** (`switch.rs`): pipeline swap metadata only — replaces **`ShellTransition`**. Does not duplicate **`PlayReplayContext`** atomics or embed **`VideoSurface`**.
- **`ShellTransition`**: legacy fat bag (swap + replay atomics + surface); **removed** when A4 lands.
- **`playback/replay.rs`**: **`PlayReplayContext`** (playback atomics), **`OverlayPlayIntent`** (from `replay` + `PipelineSwapConfig`), and **`replay_asset_shell`** for AppSrc EOS replay. Async Gst closures use **`PlaybackGstContext::clone_for_async()`** (`PlaybackGstAsyncSnapshot`).
- **`PlaybackEngine`:** holds **`Arc<PlaybackGstContext>`** (canonical `shell` + `surface`); registers **`IosLayerBackend::from_context`** into **`VideoSurface`'s bus slot** at init; load/play/notify build overlay intent via **`gst_context.overlay_intent()`**.
- `VideoSurface` (`playback/surface.rs`): `stored` + platform **`OverlaySession`**; delegates notify/apply/rebind/load-preroll. **`AndroidOverlayState` / `IosOverlayState` removed.**
- **`OverlayPrerollGate`** (`overlay/preroll/gate.rs`): pure `decide_preroll_action`. **Bind path:** **`PrerollExecutor`** + **`PrerollEffects`**. **Load path:** **`OverlaySession::apply_load_preroll`** — session computes **`gate_ready_for_load`** internally; Android load/bind share **`android_pause_preroll_with_refresh`** (`platform/android/ops.rs`).
- `PipelineCapabilities` (`playback/capabilities.rs`) types playbin-only features (seek, tracks, orientation); AppSrc pipelines report reduced capability.
- Playbin track lists are cached on bus `StreamCollection` / `StreamsSelected` and enriched from `GstStream::tags()` — playbin3 does not expose legacy `n-audio` properties.

## macOS VideoOverlay requirements (removed — historical)

<details>
<summary>Legacy Platform View + osxvideosink notes (pre-1.4)</summary>

- `FlutterPlatformViewFactory` must implement `createArgsCodec()` so `creationParams.playerId` is decoded (otherwise `playerId` stays 0 and overlay never binds).
- Bind the `NSView*` handle before the pipeline reaches `PAUSED` (proactive `set_window_handle`); `set_uri` triggers `READY → PAUSED` immediately.
- Answer `prepare-window-handle` in the pipeline bus sync handler synchronously on Android/Win/Linux; on **macOS** and **iOS** return **Pass** — bind runs on the main thread (`MacosOverlayBackend` / `IosOverlaySession`), not from the Gst bus sync handler.
- Proactive binding calls `set_window_handle` only — do not call `prepare_window_handle()` from the application side.
- Cache the `NSView*` handle synchronously in `native_window` from Swift C ABI entry points (`player_set_video_overlay_window`, `player_sync_macos_video_layer`).
- Apply the GStreamer overlay bind on the **main thread** via `DispatchQueue.main.async` (`player_apply_macos_overlay_gstreamer`). `osxvideosink` calls `setView:` directly on the main thread; calling from a background thread blocks with `performSelector:waitUntilDone:YES` and deadlocks with Flutter's merged UI/platform thread.
- `player_apply_macos_overlay_gstreamer` must call `set_window_handle` **directly on the main thread** using a cached `overlay_sink` clone — it must **not** use `spawn_on_gst_thread_and_wait` (pipeline ops stay on xhvp-gst; VideoOverlay apply is the exception).
- `play()` / `set_uri()` verify the overlay handle is cached and **bound** (`overlay_bound`); if cached but unbound, `apply_macos_overlay_gstreamer` binds on the main thread then resumes on `xhvp-gst` via `maybe_resume_after_overlay_bind`. Do not call `apply_macos_overlay_gstreamer` from the FRB thread pool for VideoOverlay I/O itself — the apply entry is main-thread (Swift) / discpatches main sync for rebind.
- Do **not** use a dedicated overlay background thread combined with `drain_overlay_queue()` — that creates a circular wait with `osxvideosink`'s main-thread dispatch.
- HTTPS requires the GIO OpenSSL TLS backend (`register_gio_tls_backend()` after `gst::init()`); without it `souphttpsrc` delivers zero bytes.
- Use a child `NSView` (`wantsLayer = false`) as the VideoOverlay target inside the Flutter platform view.

</details>

## Deprecated / removed

- Platform View factories (iOS/macOS/Android) and Windows/Linux desktop overlay HWND/GTK popups — replaced by Flutter **`Texture`**
- `VideoSurfaceKind.platformView`, `VideoSurfaceKind.desktopOverlay`, `DesktopVideoOverlay`, `DesktopOverlayClient`
- `d3d11videosink`, `osxvideosink`, `avsamplebufferlayersink` as primary sinks on desktop/Apple
- `irondash_texture`, `irondash_engine_context`

## References

- [GStreamer gst-docs](https://github.com/GStreamer/gst-docs)
- [Android tutorial 3: Video](https://gstreamer.freedesktop.org/documentation/tutorials/android/video.html)
