# Android NDK CMake for libxue_hua_video_player.so
cmake_minimum_required(VERSION 3.18)
project(xue_hua_video_player_android C)

set(CMAKE_C_STANDARD 11)
add_compile_definitions(XHVP_BUILDING __ANDROID__)

set(NATIVE_ROOT ${CMAKE_CURRENT_SOURCE_DIR}/../native)

add_library(xue_hua_video_player SHARED
  ${NATIVE_ROOT}/src/runtime.c
  ${NATIVE_ROOT}/src/bus.c
  ${NATIVE_ROOT}/src/frame.c
  ${NATIVE_ROOT}/src/pipeline.c
  ${NATIVE_ROOT}/src/xhvp_player.c
  ${NATIVE_ROOT}/src/android_jni.c
)

target_include_directories(xue_hua_video_player PRIVATE
  ${NATIVE_ROOT}/include
  ${NATIVE_ROOT}/src
)

# GStreamer Android SDK paths (set by Gradle).
if(NOT DEFINED GSTREAMER_ROOT_ANDROID)
  message(FATAL_ERROR "GSTREAMER_ROOT_ANDROID must be set")
endif()

# GStreamer Android SDK uses arm64/armv7; NDK uses arm64-v8a/armeabi-v7a.
if(ANDROID_ABI STREQUAL "arm64-v8a")
  set(GST_ABI_DIR "arm64")
elseif(ANDROID_ABI STREQUAL "armeabi-v7a")
  set(GST_ABI_DIR "armv7")
else()
  set(GST_ABI_DIR "${ANDROID_ABI}")
endif()
set(GST_ABI_ROOT "${GSTREAMER_ROOT_ANDROID}/${GST_ABI_DIR}")
target_include_directories(xue_hua_video_player PRIVATE
  ${GSTREAMER_ROOT_ANDROID}/include
  ${GST_ABI_ROOT}/include/gstreamer-1.0
  ${GST_ABI_ROOT}/include/glib-2.0
  ${GST_ABI_ROOT}/lib/glib-2.0/include
)

# Prefer umbrella shared lib produced into jniLibs by buildGstreamerUmbrella.
if(DEFINED GSTREAMER_JNI_LIBS)
  target_link_directories(xue_hua_video_player PRIVATE
    ${GSTREAMER_JNI_LIBS}/${ANDROID_ABI}
  )
endif()

find_library(log-lib log)
find_library(android-lib android)

target_link_libraries(xue_hua_video_player PRIVATE
  ${log-lib}
  ${android-lib}
  gstreamer_android
)
