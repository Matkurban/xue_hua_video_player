# 01 — Platform View + GStreamer sinks

Status: ready-for-human

## Scope

- Remove irondash_texture / irondash_engine_context
- Implement Platform View per platform
- Wire VideoOverlay window handles to playbin3 video sink
- Update Dart API (remove textureId, use Platform View)

## Review conclusion (2026-07-06)

- P0: Win/Linux CMake now links `${PROJECT_NAME}_cargokit_lib`; Windows uses MethodChannel desktop overlay (Flutter PlatformView C++ API not public); Linux uses GTK popup overlay via MethodChannel (no `FlPlatformView` in current Flutter SDK).
- P1: `pending_overlays` buffers overlay before `createPlayer`; iOS `layoutSubviews` resize; `XueHuaVideoView` fills aspect-ratio box; Android ANativeWindow release path logged.
- P2: README.md / README.zh-CN.md synced to Platform View architecture; version unified to 1.1.0; macOS `opengl` plugin added for glimagesink fallback.

## Comments
