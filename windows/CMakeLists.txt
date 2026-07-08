cmake_minimum_required(VERSION 3.14)

set(PROJECT_NAME "xue_hua_video_player")
project(${PROJECT_NAME} LANGUAGES CXX)

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

include("../cargokit/cmake/cargokit.cmake")
apply_cargokit(${PROJECT_NAME} ../rust xue_hua_video_player "")

set(PLUGIN_NAME "xue_hua_video_player_plugin")

add_library(${PLUGIN_NAME} SHARED
  "xue_hua_video_player_plugin.cpp"
  "xue_hua_video_texture.cpp"
)
apply_standard_settings(${PLUGIN_NAME})
set_target_properties(${PLUGIN_NAME} PROPERTIES CXX_VISIBILITY_PRESET hidden)
target_compile_definitions(${PLUGIN_NAME} PRIVATE FLUTTER_PLUGIN_IMPL)
target_include_directories(${PLUGIN_NAME} INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(${PLUGIN_NAME} PRIVATE flutter flutter_wrapper_plugin)
target_link_libraries(${PLUGIN_NAME} PRIVATE "${${PROJECT_NAME}_cargokit_lib}")

file(GLOB GSTREAMER_RUNTIME_DLLS "${GSTREAMER_ROOT}/bin/*.dll")

set(xue_hua_video_player_bundled_libraries
  "${${PROJECT_NAME}_cargokit_lib}"
  $<TARGET_FILE:${PLUGIN_NAME}>
  ${GSTREAMER_RUNTIME_DLLS}
  PARENT_SCOPE
)
