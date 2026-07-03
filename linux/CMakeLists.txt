# The Flutter tooling requires that developers have CMake 3.10 or later
# installed. You should not increase this version, as doing so will cause
# the plugin to fail to compile for some customers of the plugin.
cmake_minimum_required(VERSION 3.10)

# Project-level configuration.
set(PROJECT_NAME "xue_hua_video_player")
project(${PROJECT_NAME} LANGUAGES CXX)

# --- GStreamer discovery ----------------------------------------------------
# The Rust core links against GStreamer via pkg-config. Verify the development
# packages are installed so the build fails early with a clear message:
#   sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
#     gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav
find_package(PkgConfig REQUIRED)
pkg_check_modules(GST REQUIRED
  gstreamer-1.0
  gstreamer-app-1.0
  gstreamer-video-1.0
)

include("../cargokit/cmake/cargokit.cmake")
apply_cargokit(${PROJECT_NAME} ../rust xue_hua_video_player "")

# List of absolute paths to libraries that should be bundled with the plugin.
# The GStreamer runtime libraries are expected to be provided by the system
# (installed via the distro packages above); only the cargokit-built library is
# bundled here.
set(xue_hua_video_player_bundled_libraries
  "${${PROJECT_NAME}_cargokit_lib}"
  PARENT_SCOPE
)
