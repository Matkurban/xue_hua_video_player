#!/usr/bin/env bash
# Downloads and extracts the official GStreamer Android universal SDK into the
# user cache. No sudo required.
#
# Cache layout (top-level per ABI):
#   arm64/ armv7/ x86/ x86_64/
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=gstreamer_paths.sh
source "${SCRIPT_DIR}/gstreamer_paths.sh"

is_sdk_valid() {
  local root="$1"
  [[ -f "${root}/arm64/lib/libgstreamer-1.0.a" ]] \
    && [[ -f "${root}/armv7/lib/libgstreamer-1.0.a" ]] \
    && [[ -f "${root}/x86/lib/libgstreamer-1.0.a" ]] \
    && [[ -f "${root}/x86_64/lib/libgstreamer-1.0.a" ]]
}

write_stamp() {
  mkdir -p "${XUE_HUA_GSTREAMER_ANDROID_ROOT}"
  {
    echo "version=${GST_VER}"
    echo "root=${XUE_HUA_GSTREAMER_ANDROID_ROOT}"
    echo "installed_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  } > "${STAMP}"
}

if is_sdk_valid "${XUE_HUA_GSTREAMER_ANDROID_ROOT}"; then
  echo "[xue_hua_video_player] GStreamer Android ${GST_VER} cache OK at ${XUE_HUA_GSTREAMER_ANDROID_ROOT}"
  exit 0
fi

if [[ "${XUE_HUA_GSTREAMER_ANDROID_ROOT_IS_CUSTOM}" == "1" ]]; then
  echo "error: custom GStreamer Android path is incomplete at ${XUE_HUA_GSTREAMER_ANDROID_ROOT}" >&2
  echo "  Expected per-ABI dirs arm64/ armv7/ x86/ x86_64/ with lib/libgstreamer-1.0.a" >&2
  exit 1
fi

if [[ -e "${XUE_HUA_GSTREAMER_ANDROID_ROOT}" ]]; then
  echo "[xue_hua_video_player] Removing incomplete GStreamer Android cache at ${XUE_HUA_GSTREAMER_ANDROID_ROOT}..."
  rm -rf "${XUE_HUA_GSTREAMER_ANDROID_ROOT}"
fi

WORK_DIR="$(mktemp -d)"
cleanup() { rm -rf "${WORK_DIR}"; }
trap cleanup EXIT

TARBALL="${WORK_DIR}/${GSTREAMER_ANDROID_TARBALL}"

echo "[xue_hua_video_player] Downloading GStreamer Android ${GST_VER} universal SDK..."
curl -fL --retry 3 --retry-delay 2 \
  "${GSTREAMER_ANDROID_TARBALL_URL}" -o "${TARBALL}"

mkdir -p "${XUE_HUA_GSTREAMER_ANDROID_ROOT}"
echo "[xue_hua_video_player] Extracting to ${XUE_HUA_GSTREAMER_ANDROID_ROOT}..."
# NOTE: no --strip-components; tarball top level is arm64/ armv7/ x86/ x86_64/
tar -xf "${TARBALL}" -C "${XUE_HUA_GSTREAMER_ANDROID_ROOT}"

if ! is_sdk_valid "${XUE_HUA_GSTREAMER_ANDROID_ROOT}"; then
  echo "error: GStreamer Android SDK extraction failed at ${XUE_HUA_GSTREAMER_ANDROID_ROOT}" >&2
  exit 1
fi

write_stamp
echo "[xue_hua_video_player] GStreamer Android ${GST_VER} ready at ${XUE_HUA_GSTREAMER_ANDROID_ROOT}"
