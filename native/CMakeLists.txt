cmake_minimum_required(VERSION 3.10)
project(xue_hua_video_player_native C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_VISIBILITY_PRESET hidden)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
add_compile_definitions(XHVP_BUILDING)

set(XHVP_SOURCES
  src/runtime.c
  src/bus.c
  src/frame.c
  src/pipeline.c
  src/xhvp_player.c
  src/xhvp_ffi_keep.c
)

if(APPLE)
  list(APPEND XHVP_SOURCES src/apple_env.c)
endif()

add_library(xue_hua_video_player SHARED ${XHVP_SOURCES})
target_include_directories(xue_hua_video_player
  PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/include
  PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

if(ANDROID)
  find_library(log-lib log)
  find_library(android-lib android)
  # Expect GSTREAMER_ROOT_ANDROID and ABI-specific umbrella already built.
  if(NOT DEFINED GSTREAMER_ROOT_ANDROID)
    message(FATAL_ERROR "GSTREAMER_ROOT_ANDROID is required for Android builds")
  endif()
  target_compile_definitions(xue_hua_video_player PRIVATE __ANDROID__)
  target_include_directories(xue_hua_video_player PRIVATE
    ${GSTREAMER_ROOT_ANDROID}/include
    ${GSTREAMER_ROOT_ANDROID}/include/gstreamer-1.0
    ${GSTREAMER_ROOT_ANDROID}/include/glib-2.0
  )
  target_link_libraries(xue_hua_video_player PRIVATE
    ${log-lib}
    ${android-lib}
    gstreamer_android
  )
elseif(APPLE AND NOT IOS)
  # Prefer pkg-config (Homebrew) when available; else GStreamer.framework.
  find_package(PkgConfig)
  if(PkgConfig_FOUND)
    pkg_check_modules(GST gstreamer-1.0 gstreamer-app-1.0 gstreamer-video-1.0)
  endif()
  if(GST_FOUND)
    target_include_directories(xue_hua_video_player PRIVATE ${GST_INCLUDE_DIRS})
    target_link_directories(xue_hua_video_player PRIVATE ${GST_LIBRARY_DIRS})
    target_link_libraries(xue_hua_video_player PRIVATE ${GST_LIBRARIES})
  else()
    set(GST_FRAMEWORK_ROOT "$ENV{HOME}/Library/Caches/xue_hua_video_player/gstreamer/1.28.4")
    if(DEFINED ENV{GSTREAMER_FRAMEWORK_ROOT})
      set(GST_FRAMEWORK_ROOT "$ENV{GSTREAMER_FRAMEWORK_ROOT}")
    endif()
    target_include_directories(xue_hua_video_player PRIVATE
      ${GST_FRAMEWORK_ROOT}/GStreamer.framework/Headers
    )
    target_link_libraries(xue_hua_video_player PRIVATE
      "-F${GST_FRAMEWORK_ROOT}"
      "-framework GStreamer"
    )
  endif()
elseif(IOS)
  message(STATUS "iOS: link GStreamer.framework via podspec OTHER_LDFLAGS")
else()
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(GST REQUIRED gstreamer-1.0 gstreamer-app-1.0 gstreamer-video-1.0)
  target_include_directories(xue_hua_video_player PRIVATE ${GST_INCLUDE_DIRS})
  target_link_directories(xue_hua_video_player PRIVATE ${GST_LIBRARY_DIRS})
  target_link_libraries(xue_hua_video_player PRIVATE ${GST_LIBRARIES})
endif()

install(TARGETS xue_hua_video_player
  LIBRARY DESTINATION lib
  ARCHIVE DESTINATION lib
  RUNTIME DESTINATION bin
)
install(FILES include/xhvp_player.h DESTINATION include)
