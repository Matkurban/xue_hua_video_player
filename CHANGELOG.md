# Changelog

All notable changes to this project are documented in this file.

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
