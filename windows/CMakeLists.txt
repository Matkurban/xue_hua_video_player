cmake_minimum_required(VERSION 3.14)

set(PROJECT_NAME "xue_hua_video_player")
project(${PROJECT_NAME} LANGUAGES C CXX)

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

set(ENV{PKG_CONFIG_PATH} "${GSTREAMER_ROOT}/lib/pkgconfig")
find_package(PkgConfig REQUIRED)
pkg_check_modules(GST REQUIRED gstreamer-1.0 gstreamer-app-1.0 gstreamer-video-1.0)

set(PLUGIN_NAME "xue_hua_video_player_plugin")
set(NATIVE_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../native")

add_library(${PLUGIN_NAME} SHARED
  "xue_hua_video_player_plugin.cpp"
  "xue_hua_video_texture.cpp"
  "${NATIVE_ROOT}/src/runtime.c"
  "${NATIVE_ROOT}/src/bus.c"
  "${NATIVE_ROOT}/src/frame.c"
  "${NATIVE_ROOT}/src/pipeline.c"
  "${NATIVE_ROOT}/src/xhvp_player.c"
  "${NATIVE_ROOT}/src/windows_env.c"
)
apply_standard_settings(${PLUGIN_NAME})
# MSVC /WX treats C4819 as error on CP936 (Chinese) systems; force UTF-8.
target_compile_options(${PLUGIN_NAME} PRIVATE /utf-8)
set_target_properties(${PLUGIN_NAME} PROPERTIES CXX_VISIBILITY_PRESET default)
target_compile_definitions(${PLUGIN_NAME} PRIVATE
  FLUTTER_PLUGIN_IMPL
  XHVP_BUILDING
  _CRT_SECURE_NO_WARNINGS
)
target_include_directories(${PLUGIN_NAME} INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_include_directories(${PLUGIN_NAME} PRIVATE
  "${NATIVE_ROOT}/include"
  "${NATIVE_ROOT}/src"
  ${GST_INCLUDE_DIRS}
  "${GSTREAMER_ROOT}/include"
  "${GSTREAMER_ROOT}/include/gstreamer-1.0"
  "${GSTREAMER_ROOT}/include/glib-2.0"
  "${GSTREAMER_ROOT}/lib/glib-2.0/include"
)
target_link_directories(${PLUGIN_NAME} PRIVATE
  ${GST_LIBRARY_DIRS}
  "${GSTREAMER_ROOT}/lib"
)
target_link_libraries(${PLUGIN_NAME} PRIVATE flutter flutter_wrapper_plugin)
target_link_libraries(${PLUGIN_NAME} PRIVATE ${GST_LIBRARIES})

file(GLOB GSTREAMER_RUNTIME_DLLS "${GSTREAMER_ROOT}/bin/*.dll")

set(xue_hua_video_player_bundled_libraries
  $<TARGET_FILE:${PLUGIN_NAME}>
  ${GSTREAMER_RUNTIME_DLLS}
  PARENT_SCOPE
)
