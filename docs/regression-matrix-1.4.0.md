# Regression matrix — 1.4.0 (C + Dart FFI)

Manual / device checklist after the Rust → C migration. Mark when verified.

| Capability | macOS | Linux | Windows | Android | iOS |
|------------|-------|-------|---------|---------|-----|
| URI play/pause/seek | pending | pending | pending | pending | pending |
| volume/mute/speed/loop | pending | pending | pending | pending | pending |
| FlutterAsset | pending | pending | pending | pending | pending |
| Texture / SurfaceProducer | pending | pending | pending | pending | pending |
| tracks (multi-audio media) | pending | — | — | pending | pending |
| rotation/aspect | pending | pending | pending | pending | pending |
| error / EOS events | pending | pending | pending | pending | pending |
| No `n-audio` / Source ID GLib CRITICAL | pending | — | — | pending | pending |
| SPM warning absent (Flutter 3.44+) | pending | — | — | — | pending |
| Pause before surface (no auto-play) | — | — | — | pending | — |
| Release minify (ProGuard keeps) | — | — | — | pending | — |
| Rapid open/close (no ANativeWindow leak) | — | — | — | pending | — |
| Rapid scrub / dispose mid-load (no UI freeze) | pending | pending | pending | pending | pending |

## Automated (this workspace)

- [x] `dart analyze lib test` — clean
- [x] `flutter test test/media test/player test/ffi` — pass
- [x] Host `native/scripts/build_host.sh` — builds dylib
- [x] `native/scripts/build_pod.sh` — builds static `.a`
- [x] `flutter build macos --debug` — SPM detected; app embeds `1.4.0-ffi-p1`

## Notes

- Full device E2E for Android/iOS/desktop example apps should be run on developer machines with GStreamer SDKs installed.
- Gst C patches: clone `https://github.com/Matkurban/gstreamer` → `XHVP_GSTREAMER_SRC` (see `third_party/gstreamer.md`).
- Always run the example from this repo path (`XueHuaPackages/...`); clean `example/build` + Pods after native C changes.
