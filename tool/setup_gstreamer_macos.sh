#!/usr/bin/env bash
# Ensures the official universal GStreamer macOS SDK is available for builds.
#
# Default: download/extract to ~/Library/Caches/xue_hua_video_player/gstreamer/<ver>/
#   (no sudo; invoked automatically during pod install)
#
# Optional --system: install to /Library/Frameworks via sudo (maintainers only)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENSURE="${ROOT}/macos/scripts/ensure_gstreamer_macos.sh"

if [[ "${1:-}" == "--system" ]]; then
  GST_VER="${GST_VER:-1.28.4}"
  BASE="https://gstreamer.freedesktop.org/data/pkg/osx/${GST_VER}"
  RUNTIME_PKG="gstreamer-1.0-${GST_VER}-universal.pkg"
  DEVEL_PKG="gstreamer-1.0-devel-${GST_VER}-universal.pkg"

  WORK_DIR="$(mktemp -d)"
  cleanup() { rm -rf "$WORK_DIR"; }
  trap cleanup EXIT

  echo "Downloading GStreamer ${GST_VER} universal packages..."
  curl -fL --retry 3 --retry-delay 2 "${BASE}/${RUNTIME_PKG}" -o "${WORK_DIR}/${RUNTIME_PKG}"
  curl -fL --retry 3 --retry-delay 2 "${BASE}/${DEVEL_PKG}" -o "${WORK_DIR}/${DEVEL_PKG}"

  echo "Installing runtime package (requires sudo)..."
  sudo installer -pkg "${WORK_DIR}/${RUNTIME_PKG}" -target /

  echo "Installing devel package (requires sudo)..."
  sudo installer -pkg "${WORK_DIR}/${DEVEL_PKG}" -target /

  FRAMEWORK="/Library/Frameworks/GStreamer.framework"
  if [[ ! -d "${FRAMEWORK}" ]]; then
    echo "error: ${FRAMEWORK} not found after install" >&2
    exit 1
  fi

  echo "GStreamer.framework installed at ${FRAMEWORK}"
  "${FRAMEWORK}/Versions/1.0/bin/pkg-config" --modversion gstreamer-1.0
  file "${FRAMEWORK}/Versions/1.0/lib/libgstreamer-1.0.0.dylib"
  exit 0
fi

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  echo "Usage: sh tool/setup_gstreamer_macos.sh [--system]"
  echo "  (default) Ensure GStreamer in user cache (no sudo)"
  echo "  --system  Install to /Library/Frameworks (sudo, optional)"
  exit 0
fi

exec sh "${ENSURE}"
