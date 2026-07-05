#!/usr/bin/env bash
# Removes GStreamer plugins not in gstreamer_playback_plugins.txt from a framework copy.
# Usage: prune_gstreamer_plugins.sh /path/to/GStreamer.framework
set -euo pipefail

FRAMEWORK="${1:?framework path required}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MACOS_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
WHITELIST="${MACOS_DIR}/gstreamer_playback_plugins.txt"
PLUGIN_DIR="${FRAMEWORK}/Versions/1.0/lib/gstreamer-1.0"

if [[ ! -d "${PLUGIN_DIR}" ]]; then
  echo "error: plugin dir not found: ${PLUGIN_DIR}" >&2
  exit 1
fi

if [[ ! -f "${WHITELIST}" ]]; then
  echo "error: whitelist not found: ${WHITELIST}" >&2
  exit 1
fi

is_whitelisted() {
  local name="$1"
  local line
  while IFS= read -r line || [[ -n "${line}" ]]; do
    line="${line%%#*}"
    line="$(echo "${line}" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
    [[ -z "${line}" ]] && continue
    if [[ "${line}" == "${name}" ]]; then
      return 0
    fi
  done < "${WHITELIST}"
  return 1
}

removed=0
kept=0
for plugin in "${PLUGIN_DIR}"/libgst*.dylib; do
  [[ -e "${plugin}" ]] || continue
  base="$(basename "${plugin}")"
  name="${base#libgst}"
  name="${name%.dylib}"
  if is_whitelisted "${name}"; then
    kept=$((kept + 1))
  else
    rm -f "${plugin}"
    removed=$((removed + 1))
  fi
done

echo "[xue_hua_video_player] Pruned GStreamer plugins: kept ${kept}, removed ${removed}"
