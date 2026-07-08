cmake_minimum_required(VERSION 3.10)

set(PROJECT_NAME "xue_hua_video_player")
project(${PROJECT_NAME} LANGUAGES CXX)

find_package(PkgConfig REQUIRED)
pkg_check_modules(GST REQUIRED
  gstreamer-1.0
  gstreamer-app-1.0
  gstreamer-video-1.0
)
pkg_check_modules(GTK REQUIRED gtk+-3.0)

include("../cargokit/cmake/cargokit.cmake")
apply_cargokit(${PROJECT_NAME} ../rust xue_hua_video_player "")

set(PLUGIN_NAME "xue_hua_video_player_plugin")

add_library(${PLUGIN_NAME} SHARED
  "xue_hua_video_player_plugin.cc"
  "xue_hua_video_texture.cc"
)
apply_standard_settings(${PLUGIN_NAME})
set_target_properties(${PLUGIN_NAME} PROPERTIES CXX_VISIBILITY_PRESET hidden)
target_compile_definitions(${PLUGIN_NAME} PRIVATE FLUTTER_PLUGIN_IMPL)
target_include_directories(${PLUGIN_NAME} INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(${PLUGIN_NAME} PRIVATE flutter)
target_link_libraries(${PLUGIN_NAME} PRIVATE PkgConfig::GTK)
target_link_libraries(${PLUGIN_NAME} PRIVATE "${${PROJECT_NAME}_cargokit_lib}")

set(xue_hua_video_player_bundled_libraries
  "${${PROJECT_NAME}_cargokit_lib}"
  $<TARGET_FILE:${PLUGIN_NAME}>
  PARENT_SCOPE
)
