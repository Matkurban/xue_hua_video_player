## 1.0.1

### Bug fixes

- Fixed replay after playback reaches the end of the media: pressing play again
  now seeks back to the start instead of staying stuck at the end.
- Buffering events now set `PlayerState.buffering` so the control bar shows the
  loading indicator during network buffering.

### Improvements

- Playback speed changes now preserve the original pitch (via GStreamer's
  `scaletempo` audio filter) instead of shifting tone at non-1x rates.
- Optimized control bar UI.

### Platform notes

- Android: bundled `audiofx` plugin (includes `scaletempo`) in
  `libgstreamer_android.so` so pitch-preserving speed works out of the box.
- Other platforms: pitch-preserving speed uses the system `audiofx` / `plugins-good`
  package (`gstreamer1.0-plugins-good` on Linux, GStreamer MSVC good plugins on
  Windows, Homebrew `gst-plugins-good` on macOS). If `scaletempo` is unavailable,
  playback speed falls back to the previous pitch-shifting behaviour.

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
