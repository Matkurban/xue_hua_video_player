# The Flutter tooling requires that developers have a version of Visual Studio
# installed that includes CMake 3.14 or later. You should not increase this
# version, as doing so will cause the plugin to fail to compile for some
# customers of the plugin.
cmake_minimum_required(VERSION 3.14)

# Project-level configuration.
set(PROJECT_NAME "xue_hua_video_player")
project(${PROJECT_NAME} LANGUAGES CXX)

# --- GStreamer discovery ----------------------------------------------------
# Locate the GStreamer MSVC install. The official installer sets the environment
# variable GSTREAMER_1_0_ROOT_MSVC_X86_64 (install both the "runtime" and
# "development" packages from https://gstreamer.freedesktop.org/download/).
if(DEFINED ENV{GSTREAMER_1_0_ROOT_MSVC_X86_64})
  file(TO_CMAKE_PATH "$ENV{GSTREAMER_1_0_ROOT_MSVC_X86_64}" GSTREAMER_ROOT)
else()
  set(GSTREAMER_ROOT "C:/gstreamer/1.0/msvc_x86_64")
endif()

if(NOT EXISTS "${GSTREAMER_ROOT}/bin")
  message(FATAL_ERROR
    "GStreamer not found at '${GSTREAMER_ROOT}'. Install the GStreamer MSVC "
    "runtime + development packages and/or set GSTREAMER_1_0_ROOT_MSVC_X86_64.")
endif()

# Let the Rust build (system-deps/pkg-config) locate GStreamer. Requires a
# pkg-config/pkgconf binary on PATH (e.g. `choco install pkgconfiglite`).
set(ENV{PKG_CONFIG_PATH} "${GSTREAMER_ROOT}/lib/pkgconfig")

include("../cargokit/cmake/cargokit.cmake")
apply_cargokit(${PROJECT_NAME} ../rust xue_hua_video_player "")

# Bundle the GStreamer runtime DLLs next to the built application so it can run
# without a system-wide install. Plugin DLLs live under lib/gstreamer-1.0 and are
# located at runtime via GST_PLUGIN_SYSTEM_PATH (see README for packaging notes).
file(GLOB GSTREAMER_RUNTIME_DLLS "${GSTREAMER_ROOT}/bin/*.dll")

set(xue_hua_video_player_bundled_libraries
  "${${PROJECT_NAME}_cargokit_lib}"
  ${GSTREAMER_RUNTIME_DLLS}
  PARENT_SCOPE
)
