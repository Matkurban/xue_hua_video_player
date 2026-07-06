# PRD: GStreamer Platform Texture Migration

## Goal

Replace irondash external-texture bridge with GStreamer-recommended per-platform video sinks (`glimagesink`, `osxvideosink`, `d3d11videosink`) rendering into Flutter Platform Views via VideoOverlay.

## Success criteria

- No `irondash_texture` or `irondash_engine_context` dependencies
- Video renders on Android, iOS, macOS, Windows, Linux via Platform Views
- Playback controls (seek, pause, volume, speed) continue to work
- Hardware decode on Android via MediaCodec → glimagesink zero-copy path

## References

- GStreamer Basic tutorial 16 (platform-specific elements)
- GStreamer Android tutorial 3 (VideoOverlay + Surface)
