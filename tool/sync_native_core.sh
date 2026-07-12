#!/usr/bin/env bash
# Materialize native/ into ios|macos SPM NativeCore/ as real directory trees.
# Directory symlinks break under `dart pub publish` (become path-text stubs),
# so SPM hosts from pub.dev never compile xhvp_player_c.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC_INCLUDE="${ROOT}/native/include"
SRC_SRC="${ROOT}/native/src"

if [[ ! -d "${SRC_INCLUDE}" || ! -d "${SRC_SRC}" ]]; then
  echo "error: native sources missing under ${ROOT}/native" >&2
  exit 1
fi

sync_platform() {
  local platform="$1"
  local dest="${ROOT}/${platform}/xue_hua_video_player/NativeCore"
  mkdir -p "${dest}"

  rm -rf "${dest}/include" "${dest}/src"
  cp -R "${SRC_INCLUDE}" "${dest}/include"
  cp -R "${SRC_SRC}" "${dest}/src"

  for required in xhvp_player.c xhvp_ffi_keep.c; do
    if [[ ! -f "${dest}/src/${required}" ]]; then
      echo "error: ${platform} NativeCore missing ${required} after sync" >&2
      exit 1
    fi
  done
  if [[ ! -f "${dest}/include/xhvp_player.h" ]]; then
    echo "error: ${platform} NativeCore missing include/xhvp_player.h after sync" >&2
    exit 1
  fi

  echo "[xhvp] synced native/ -> ${platform}/xue_hua_video_player/NativeCore"
}

sync_platform ios
sync_platform macos
echo "[xhvp] NativeCore sync complete"
