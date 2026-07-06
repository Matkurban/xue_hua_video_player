#!/usr/bin/env bash
# Removes dylibs in GStreamer.framework/lib that nothing else references.
# MoltenVK and core libs are never removed.
# Usage: prune_gstreamer_orphan_dylibs.sh /path/to/GStreamer.framework
set -euo pipefail

FRAMEWORK="${1:?framework path required}"
LIB_DIR="${FRAMEWORK}/Versions/1.0/lib"

if [[ ! -d "${LIB_DIR}" ]]; then
  echo "error: lib dir not found: ${LIB_DIR}" >&2
  exit 1
fi

is_protected() {
  case "$1" in
    libMoltenVK.dylib|libgstreamer-1.0.0.dylib|libgstbase-1.0.0.dylib|\
    libgstreamer-1.0.dylib|libgstbase-1.0.dylib|libglib-2.0.0.dylib|\
    libgobject-2.0.0.dylib|libgio-2.0.0.dylib|libgmodule-2.0.0.dylib|\
    libgiolibopenssl.so|libgioopenssl.so|\
    libintl.8.dylib|liborc-0.4.0.dylib|libavcodec.61.dylib|libavformat.61.dylib|\
    libavutil.59.dylib|libswresample.5.dylib|libcrypto.3.dylib|libssl.3.dylib)
      return 0
      ;;
  esac
  return 1
}

raw_refs="$(mktemp)"
refs_file="$(mktemp)"
while IFS= read -r macho; do
  otool -L "${macho}" 2>/dev/null | tail -n +2 | awk '{print $1}' >> "${raw_refs}" || true
done < <(find "${FRAMEWORK}" \( -name '*.dylib' -o -name '*.so' \) -type f)

while IFS= read -r dep; do
  [[ -n "${dep}" ]] || continue
  basename "${dep}"
done < "${raw_refs}" | sort -u > "${refs_file}"
rm -f "${raw_refs}"

removed=0
for dylib in "${LIB_DIR}"/*.dylib; do
  [[ -e "${dylib}" ]] || continue
  base="$(basename "${dylib}")"
  if is_protected "${base}"; then
    continue
  fi
  if ! grep -Fxq "${base}" "${refs_file}" 2>/dev/null; then
    rm -f "${dylib}"
    removed=$((removed + 1))
    echo "[xue_hua_video_player] Removed orphan dylib: ${base}"
  fi
done

rm -f "${refs_file}"
echo "[xue_hua_video_player] Orphan dylib cleanup removed ${removed} file(s)"
