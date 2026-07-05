#!/usr/bin/env bash
# Copies and slim-down the runtime GStreamer.framework into macos/Vendored/ so
# CocoaPods vendored_frameworks + [CP] Embed Pods Frameworks copies it into .app.
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
cp -Rc "${RUNTIME_SRC}" "${VENDORED_FW}"

bash "${SCRIPT_DIR}/strip_gstreamer_runtime.sh" "${VENDORED_FW}"
bash "${SCRIPT_DIR}/prune_gstreamer_plugins.sh" "${VENDORED_FW}"
bash "${SCRIPT_DIR}/prune_gstreamer_orphan_dylibs.sh" "${VENDORED_FW}"
bash "${SCRIPT_DIR}/thin_gstreamer_framework.sh" "${VENDORED_FW}"

size="$(du -sh "${VENDORED_FW}" | awk '{print $1}')"
echo "[xue_hua_video_player] Vendored slim GStreamer.framework (${size}) from ${RUNTIME_SRC}"
