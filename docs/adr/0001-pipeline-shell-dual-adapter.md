# ADR-0001: PipelineShell with dual source adapters

## Status

Accepted (2026-07-06)

## Context

The player must support network URLs, local `file://` paths, and Flutter bundle assets. GStreamer offers two practical wiring patterns:

1. **playbin3** for URI sources (HTTP, RTSP, HLS, `file://`)
2. **AppSrc → decodebin** for Android `AssetManager` FDs and other non-URI asset byte streams

An earlier layout duplicated sink creation, bus handlers, and VideoOverlay wiring in separate modules (`pipeline_builder.rs`, `asset_appsrc.rs`, `gst_player.rs`).

## Decision

Introduce a shared **`PipelineShell`** (`rust/src/playback/shell.rs`) that owns:

- platform `video_sink` + `audio_bin` (+ optional `text_sink` fakesink for subtitle track metadata)
- bus watch + position polling (`playback/bus.rs`)
- overlay sync handler (`video/overlay.rs`)

Two **adapters** install sources into the shell:

| Adapter | Module | When |
|---------|--------|------|
| UriSourceAdapter | `playback/uri_pipeline.rs` | `ResolvedSource::Uri` |
| AssetSourceAdapter | `playback/asset_pipeline.rs` | `ResolvedSource::AppSrc` |

`MediaSource::resolve()` (`media/mod.rs`) picks the adapter:

- Desktop assets with on-disk `flutter_assets/` paths → `file://` + playbin3 (full seek)
- Android assets → AppSrc via `AssetManager.openFd`
- Network/local paths → normalized URI + playbin3

The public Rust seam is **`PlaybackEngine`** (`playback/engine.rs`); FRB/Dart call through `api/player.rs`.

## Consequences

- Bus/overlay logic is written once; platform overlay constraints in `CONTEXT.md` remain centralized.
- Asset seek is limited for pure AppSrc streams; `is_seekable` is exposed to Dart.
- Subtitle **track enumeration** on URI/playbin3 pipelines uses `GstStreamCollection` bus messages and `GST_EVENT_SELECT_STREAMS` (not legacy playbin `n-audio` properties). Burned-in rendering is out of scope for v1.
