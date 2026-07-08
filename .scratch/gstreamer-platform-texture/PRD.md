# PRD: GStreamer Platform Texture Migration

## Goal

Replace Platform View + VideoOverlay desktop/mobile embedding with Flutter
external **`Texture`** rendering on all five supported platforms, using a
custom native bridge (no third-party texture plugins).

## Success criteria

- Video renders on Android, iOS, macOS, Windows, Linux via Flutter `Texture`
- No Platform View factories or Win/Linux GTK/HWND overlay popups
- Playback controls (seek, pause, volume, speed) continue to work
- Android keeps hardware decode via MediaCodec → `glimagesink` into
  `SurfaceProducer` (zero-copy GL path)

## Architecture (landed)

| Platform | GStreamer sink | Flutter integration |
|----------|----------------|---------------------|
| Android | `glimagesink` | `TextureRegistry.SurfaceProducer` → `ANativeWindow` |
| iOS / macOS | `appsink` (BGRA) | `FlutterTexture` + IOSurface `CVPixelBuffer` |
| Windows / Linux | `appsink` (BGRA) | `PixelBufferTexture` / `FlPixelBufferTexture` (RGBA) |

Rust `FrameSink` + C-ABI (`xhvp_texture_*`) feeds Apple/desktop textures.
Android uses existing `AndroidOverlaySession` with JNI surface bind.

## References

- GStreamer Android tutorial 3 (VideoOverlay + Surface)
- Flutter engine `TextureRegistry` / `SurfaceProducer` APIs
