#!/usr/bin/env bash
# Copies the runtime GStreamer.framework snapshot into macos/Vendored/ so
# CocoaPods vendored_frameworks + [CP] Embed Pods Frameworks copies it into .app
# as GStreamer.framework (symlinks would embed as GStreamerRuntime.framework).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MACOS_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
VENDORED_DIR="${MACOS_DIR}/Vendored"
VENDORED_FW="${VENDORED_DIR}/GStreamer.framework"

# shellcheck source=gstreamer_paths.sh
source "${SCRIPT_DIR}/gstreamer_paths.sh"

if [[ "${XUE_HUA_ALLOW_HOMEBREW_GSTREAMER:-}" == "1" ]]; then
  rm -rf "${VENDORED_DIR}"
  exit 0
fi

RUNTIME_SRC="${GSTREAMER_RUNTIME_FRAMEWORK_SRC}"
if [[ ! -f "${RUNTIME_SRC}/Versions/1.0/lib/libgstreamer-1.0.0.dylib" ]]; then
  echo "error: runtime GStreamer.framework not found at ${RUNTIME_SRC}" >&2
  echo "Run: sh macos/scripts/ensure_gstreamer_macos.sh" >&2
  exit 1
fi

mkdir -p "${VENDORED_DIR}"
rm -rf "${VENDORED_FW}"
# Copy (APFS clone when possible) so CocoaPods embeds as GStreamer.framework, not the
# runtime snapshot name GStreamerRuntime.framework.
cp -Rc "${RUNTIME_SRC}" "${VENDORED_FW}"

echo "[xue_hua_video_player] Vendored GStreamer.framework from ${RUNTIME_SRC}"
