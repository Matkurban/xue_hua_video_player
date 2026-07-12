#!/usr/bin/env bash
# Build static libxue_hua_video_player.a for CocoaPods (macOS/iOS).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="${PODS_CONFIGURATION_BUILD_DIR:-${ROOT}/build/pod}/xue_hua_video_player"
mkdir -p "${OUT_DIR}"

export PATH="/opt/homebrew/bin:/usr/local/bin:${PATH}"

SRCS=(
  "${ROOT}/src/runtime.c"
  "${ROOT}/src/bus.c"
  "${ROOT}/src/frame.c"
  "${ROOT}/src/pipeline.c"
  "${ROOT}/src/thumbnail.c"
  "${ROOT}/src/xhvp_player.c"
  "${ROOT}/src/xhvp_ffi_keep.c"
)

if [[ "$(uname -s)" == "Darwin" ]]; then
  SRCS+=("${ROOT}/src/apple_env.c")
fi

if [[ "${PLATFORM_NAME:-macosx}" == "iphoneos" || "${PLATFORM_NAME:-}" == "iphonesimulator" || "${XHVP_IOS:-}" == "1" ]]; then
  SRCS+=("${ROOT}/src/ios_plugins.c")
  SRCS+=("${ROOT}/src/ios_tls.c")
fi

OBJDIR="${OUT_DIR}/obj"
mkdir -p "${OBJDIR}"

CFLAGS=(
  -std=c11
  -fPIC
  -O2
  -DXHVP_BUILDING
  -I"${ROOT}/include"
  -I"${ROOT}/src"
  -Wall
  -Wno-unused-parameter
)

# Prefer GStreamer.framework headers from cache / env.
GST_VER="${GST_VER:-1.28.4}"
GST_CACHE="${HOME}/Library/Caches/xue_hua_video_player/gstreamer/${GST_VER}"
if [[ -n "${GSTREAMER_FRAMEWORK_ROOT:-}" ]]; then
  GST_CACHE="${GSTREAMER_FRAMEWORK_ROOT}"
fi

if [[ "${PLATFORM_NAME:-macosx}" == "iphoneos" || "${PLATFORM_NAME:-}" == "iphonesimulator" || "${XHVP_IOS:-}" == "1" ]]; then
  # iOS: headers from GStreamer.framework; linking done in podspec.
  if [[ -d "${GST_CACHE}/GStreamer.framework/Headers" ]]; then
    CFLAGS+=(-I"${GST_CACHE}/GStreamer.framework/Headers")
  fi
  CFLAGS+=(-DTARGET_OS_IPHONE=1)
elif command -v pkg-config >/dev/null 2>&1 && pkg-config --exists gstreamer-1.0; then
  # shellcheck disable=SC2207
  CFLAGS+=($(pkg-config --cflags gstreamer-1.0 gstreamer-app-1.0 gstreamer-video-1.0))
elif [[ -d "${GST_CACHE}/GStreamer.framework/Headers" ]]; then
  CFLAGS+=(-I"${GST_CACHE}/GStreamer.framework/Headers")
else
  echo "GStreamer headers not found" >&2
  exit 1
fi

ARCHS_LIST="${ARCHS:-$(uname -m)}"
OBJS=()
for src in "${SRCS[@]}"; do
  base="$(basename "${src}" .c)"
  obj="${OBJDIR}/${base}.o"
  # shellcheck disable=SC2086
  clang -c "${CFLAGS[@]}" ${OTHER_CFLAGS:-} -o "${obj}" "${src}"
  OBJS+=("${obj}")
done

LIB="${OUT_DIR}/libxue_hua_video_player.a"
libtool -static -o "${LIB}" "${OBJS[@]}"
echo "[xhvp] built ${LIB}"
