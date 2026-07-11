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
# Drop CLI tools (bin), nested helpers (libexec), devel headers/share, and
# fontconfig etc (conf.d links into share/fontconfig — MAS ITMS-90332 if left
# dangling). Keep etc/ssl for CA certs. Playback uses FONTCONFIG_PATH=tmp.
rm -rf \
  "${FRAMEWORK}/Versions/1.0/include" \
  "${FRAMEWORK}/Versions/1.0/share" \
  "${FRAMEWORK}/Versions/1.0/bin" \
  "${FRAMEWORK}/Versions/1.0/libexec" \
  "${FRAMEWORK}/Versions/1.0/lib/pkgconfig" \
  "${FRAMEWORK}/Versions/1.0/etc/fonts" \
  2>/dev/null || true

# Commands -> bin (removed). Headers -> Versions/Current/Headers (runtime has
# no Headers dir). Leaving them fails MAS ITMS-90332.
rm -f \
  "${FRAMEWORK}/Commands" \
  "${FRAMEWORK}/Headers" \
  "${FRAMEWORK}/Versions/1.0/Commands"

# Defense: any remaining dangling symlink anywhere in the framework.
dangling=0
while IFS= read -r -d '' link; do
  rm -f "${link}"
  dangling=$((dangling + 1))
  echo "[xue_hua_video_player] Removed dangling symlink: ${link#${FRAMEWORK}/}"
done < <(find "${FRAMEWORK}" -type l ! -exec test -e {} \; -print0 2>/dev/null)
if [[ "${dangling}" -gt 0 ]]; then
  echo "[xue_hua_video_player] Framework dangling symlink cleanup removed ${dangling} link(s)"
fi
