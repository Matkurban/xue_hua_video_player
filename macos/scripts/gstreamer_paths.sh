#!/usr/bin/env bash
# Shared path resolution for macOS GStreamer integration.
# Source this file from other scripts; do not execute directly.

GST_VER="${GST_VER:-1.28.4}"

_default_cache_root="${HOME}/Library/Caches/xue_hua_video_player/gstreamer/${GST_VER}"

if [[ -n "${XUE_HUA_GSTREAMER_ROOT:-}" ]]; then
  XUE_HUA_GSTREAMER_CACHE="${XUE_HUA_GSTREAMER_ROOT}"
else
  XUE_HUA_GSTREAMER_CACHE="${_default_cache_root}"
fi

# GSTREAMER_FRAMEWORK_SRC (full SDK): explicit > cache > legacy system install
if [[ -n "${GSTREAMER_FRAMEWORK_SRC:-}" ]]; then
  :
elif [[ -d "${XUE_HUA_GSTREAMER_CACHE}/GStreamer.framework" ]]; then
  GSTREAMER_FRAMEWORK_SRC="${XUE_HUA_GSTREAMER_CACHE}/GStreamer.framework"
elif [[ -d "/Library/Frameworks/GStreamer.framework" ]]; then
  GSTREAMER_FRAMEWORK_SRC="/Library/Frameworks/GStreamer.framework"
  if [[ -z "${XUE_HUA_GSTREAMER_ROOT:-}" ]]; then
    XUE_HUA_GSTREAMER_CACHE="/Library/Frameworks"
  fi
else
  GSTREAMER_FRAMEWORK_SRC="${XUE_HUA_GSTREAMER_CACHE}/GStreamer.framework"
fi

# GSTREAMER_RUNTIME_FRAMEWORK_SRC (embed-only): explicit > cache runtime snapshot
if [[ -n "${GSTREAMER_RUNTIME_FRAMEWORK_SRC:-}" ]]; then
  :
elif [[ -d "${XUE_HUA_GSTREAMER_CACHE}/GStreamerRuntime.framework" ]]; then
  GSTREAMER_RUNTIME_FRAMEWORK_SRC="${XUE_HUA_GSTREAMER_CACHE}/GStreamerRuntime.framework"
else
  GSTREAMER_RUNTIME_FRAMEWORK_SRC="${XUE_HUA_GSTREAMER_CACHE}/GStreamerRuntime.framework"
fi

# Directory passed to -framework / FRAMEWORK_SEARCH_PATHS (parent of GStreamer.framework)
_parent="$(dirname "${GSTREAMER_FRAMEWORK_SRC}")"
if [[ -d "${_parent}" ]]; then
  GSTREAMER_SEARCH_FRAMEWORK="$(cd "${_parent}" && pwd)"
else
  GSTREAMER_SEARCH_FRAMEWORK="${_parent}"
fi

export GST_VER XUE_HUA_GSTREAMER_CACHE GSTREAMER_FRAMEWORK_SRC GSTREAMER_RUNTIME_FRAMEWORK_SRC GSTREAMER_SEARCH_FRAMEWORK
