#!/usr/bin/env bash
# Removes build-only artifacts from an embedded GStreamer.framework copy.
# Usage: strip_gstreamer_runtime.sh /path/to/GStreamer.framework
set -euo pipefail

FRAMEWORK="${1:?framework path required}"

if [[ ! -d "${FRAMEWORK}" ]]; then
  echo "error: framework not found: ${FRAMEWORK}" >&2
  exit 1
fi

find "${FRAMEWORK}" -maxdepth 1 -name '.*' -type f -delete 2>/dev/null || true

find "${FRAMEWORK}" -name '*.a' -delete 2>/dev/null || true
# Drop CLI tools (bin) and nested helpers (libexec: gst-plugin-scanner,
# gst-ptp-helper). MAS requires every nested executable to carry
# com.apple.security.app-sandbox; removing them avoids ITMS-90296 and
# sandboxed helper spawn failures. Playback uses in-process plugin scan.
rm -rf \
  "${FRAMEWORK}/Versions/1.0/include" \
  "${FRAMEWORK}/Versions/1.0/share" \
  "${FRAMEWORK}/Versions/1.0/bin" \
  "${FRAMEWORK}/Versions/1.0/libexec" \
  "${FRAMEWORK}/Versions/1.0/lib/pkgconfig" \
  2>/dev/null || true
