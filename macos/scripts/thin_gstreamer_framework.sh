#!/usr/bin/env bash
# Thins Mach-O binaries inside GStreamer.framework to a single architecture.
# Usage: thin_gstreamer_framework.sh /path/to/GStreamer.framework [arm64|x86_64|universal]
set -euo pipefail

FRAMEWORK="${1:?framework path required}"
ARCH="${2:-${XUE_HUA_GSTREAMER_ARCH:-universal}}"

if [[ "${ARCH}" == "universal" ]]; then
  echo "[xue_hua_video_player] GStreamer framework arch: universal (no thinning)"
  exit 0
fi

if [[ "${ARCH}" != "arm64" && "${ARCH}" != "x86_64" ]]; then
  echo "error: unsupported arch '${ARCH}' (use arm64, x86_64, or universal)" >&2
  exit 1
fi

if ! command -v lipo >/dev/null 2>&1; then
  echo "error: lipo not found" >&2
  exit 1
fi

thin_one() {
  local file="$1"
  if ! file "${file}" | grep -q 'Mach-O'; then
    return 0
  fi
  if ! lipo -info "${file}" 2>/dev/null | grep -q 'Architectures'; then
    return 0
  fi
  if ! lipo -info "${file}" 2>/dev/null | grep -q "${ARCH}"; then
    echo "warning: ${file} has no ${ARCH} slice; skipping" >&2
    return 0
  fi
  local tmp
  tmp="$(mktemp "${file}.thin.XXXXXX")"
  lipo -thin "${ARCH}" "${file}" -output "${tmp}"
  mv "${tmp}" "${file}"
}

count=0
find "${FRAMEWORK}" \( -name '*.dylib' -o -name '*.so' -o -name 'GStreamer' \) -type f | while IFS= read -r macho; do
  thin_one "${macho}"
  count=$((count + 1))
done

echo "[xue_hua_video_player] Thinned GStreamer.framework to ${ARCH}"
