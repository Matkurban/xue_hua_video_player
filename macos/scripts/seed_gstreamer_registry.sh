#!/usr/bin/env bash
# Build a GStreamer plugin registry against the (pruned) embedded framework so
# cold-start gst_init can skip a full in-process plugin scan.
# Usage: seed_gstreamer_registry.sh /path/to/GStreamer.framework
set -euo pipefail

FRAMEWORK="${1:?framework path required}"
LIB="${FRAMEWORK}/Versions/1.0/lib"
PLUGINS="${LIB}/gstreamer-1.0"
INSPECT="${FRAMEWORK}/Versions/1.0/bin/gst-inspect-1.0"
SEED="${LIB}/gstreamer-registry.bin.seed"

if [[ ! -d "${PLUGINS}" ]]; then
  echo "warning: no plugins dir at ${PLUGINS}; skipping registry seed" >&2
  exit 0
fi

if [[ ! -x "${INSPECT}" ]]; then
  echo "warning: gst-inspect-1.0 missing at ${INSPECT}; skipping registry seed" >&2
  exit 0
fi

TMP_REG="$(mktemp -t xhvp-gst-registry)"
rm -f "${TMP_REG}"

export GST_PLUGIN_SYSTEM_PATH="${PLUGINS}"
export GST_PLUGIN_PATH=""
export GST_REGISTRY_FORK=no
export GST_REGISTRY="${TMP_REG}"
# Avoid scanning host Homebrew plugins.
export GST_PLUGIN_SYSTEM_PATH_1_0="${PLUGINS}"

if ! "${INSPECT}" > /dev/null 2>&1; then
  echo "warning: gst-inspect failed; skipping registry seed" >&2
  rm -f "${TMP_REG}"
  exit 0
fi

if [[ ! -f "${TMP_REG}" ]]; then
  echo "warning: registry file not created; skipping seed" >&2
  exit 0
fi

cp -f "${TMP_REG}" "${SEED}"
rm -f "${TMP_REG}"
echo "[xue_hua_video_player] Seeded GStreamer registry ($(du -h "${SEED}" | awk '{print $1}'))"
