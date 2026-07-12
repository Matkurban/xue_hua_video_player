#!/usr/bin/env bash
# Assert ios|macos NativeCore contain real C trees (not pub-broken symlink stubs).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FAILED=0

check_platform() {
  local platform="$1"
  local core="${ROOT}/${platform}/xue_hua_video_player/NativeCore"
  local src="${core}/src"
  local include="${core}/include"

  if [[ -L "${src}" || -L "${include}" ]]; then
    echo "error: ${platform} NativeCore still uses symlinks; run tool/sync_native_core.sh" >&2
    FAILED=1
    return
  fi

  if [[ -f "${src}" && ! -d "${src}" ]]; then
    echo "error: ${platform} NativeCore/src is a file (pub symlink stub?), not a directory" >&2
    FAILED=1
    return
  fi
  if [[ ! -d "${src}" || ! -d "${include}" ]]; then
    echo "error: ${platform} NativeCore missing include/ or src/ directories" >&2
    FAILED=1
    return
  fi

  for required in xhvp_player.c xhvp_ffi_keep.c frame.c; do
    local f="${src}/${required}"
    if [[ ! -f "${f}" ]]; then
      echo "error: ${platform} NativeCore missing src/${required}" >&2
      FAILED=1
      continue
    fi
    local size
    size="$(wc -c < "${f}" | tr -d ' ')"
    if [[ "${size}" -lt 100 ]]; then
      echo "error: ${platform} NativeCore/src/${required} is only ${size} bytes (stub?)" >&2
      FAILED=1
    fi
  done

  if [[ ! -f "${include}/xhvp_player.h" ]]; then
    echo "error: ${platform} NativeCore missing include/xhvp_player.h" >&2
    FAILED=1
  fi
}

check_platform ios
check_platform macos

if [[ "${FAILED}" -ne 0 ]]; then
  echo "NativeCore verification failed" >&2
  exit 1
fi

echo "[xhvp] NativeCore verification OK"
