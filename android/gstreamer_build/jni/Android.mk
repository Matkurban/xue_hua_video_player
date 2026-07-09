LOCAL_PATH := $(call my-dir)

# A throwaway module that pulls in libgstreamer_android.so (which is what we
# actually want to ship). We have no JNI C of our own; the Rust cdylib links
# against libgstreamer_android.so at build time; the umbrella .so is built at
# compile time and packaged via android/build/gstreamer/jniLibs/.
include $(CLEAR_VARS)
LOCAL_MODULE    := dummy
LOCAL_SRC_FILES := dummy.c
LOCAL_SHARED_LIBRARIES := gstreamer_android
include $(BUILD_SHARED_LIBRARY)

ifndef GSTREAMER_ROOT_ANDROID
$(error GSTREAMER_ROOT_ANDROID is not defined!)
endif

ifeq ($(TARGET_ARCH_ABI),armeabi-v7a)
GSTREAMER_ROOT := $(GSTREAMER_ROOT_ANDROID)/armv7
else ifeq ($(TARGET_ARCH_ABI),arm64-v8a)
GSTREAMER_ROOT := $(GSTREAMER_ROOT_ANDROID)/arm64
else ifeq ($(TARGET_ARCH_ABI),x86)
GSTREAMER_ROOT := $(GSTREAMER_ROOT_ANDROID)/x86
else ifeq ($(TARGET_ARCH_ABI),x86_64)
GSTREAMER_ROOT := $(GSTREAMER_ROOT_ANDROID)/x86_64
else
$(error Target arch ABI not supported: $(TARGET_ARCH_ABI))
endif

GSTREAMER_NDK_BUILD_PATH := $(GSTREAMER_ROOT)/share/gst-android/ndk-build/

# Explicit, minimal plugin set for local + network video playback. We avoid the
# broad CODECS/EFFECTS groups because some of their (Rust-based) plugins pull in
# static deps the ndk-build whole-archive step can't resolve (lcevc_*, bare
# c++/m). androidmedia provides HW H.264/H.265 decode; reqwest provides http(s)
# (gst-plugins-rs reqwesthttpsrc — avoids souphttpsrc g_main_loop pthread TLS crash).
# libgstreqwest.a is rebuilt with current_thread Tokio before ndk-build; see
# android/scripts/build_reqwest_plugin_android.sh.
#
# `opengl` is REQUIRED for hardware decode: Qualcomm/Android MediaCodec decoders
# (amcvideodec) only emit GL textures (memory:GLMemory, texture-target=
# external-oes) and refuse to negotiate with a plain system-memory videoconvert
# ("Codec only supports GL output but downstream does not" -> not-negotiated).
# It provides glcolorconvert/gldownload so the video sink can accept the GL
# texture, convert it to RGBA and download it to system memory for the appsink.
GSTREAMER_PLUGINS := \
    coreelements app typefindfunctions playback \
    audioconvert audioresample audiofx videoconvertscale volume autodetect \
    isomp4 matroska audioparsers videoparsersbad id3demux \
    androidmedia videofilter opengl \
    reqwest tcp udp opensles

GSTREAMER_EXTRA_DEPS := gstreamer-video-1.0 gstreamer-app-1.0
# TLS for https:// sources. The openssl gio module needs libssl/libcrypto,
# which the ndk-build integration does not pull in automatically.
G_IO_MODULES := openssl
# -lssl/-lcrypto for the openssl TLS backend; -lEGL/-lGLESv2 for the opengl
# plugin (glcolorconvert/gldownload create an EGL context to receive the
# amcvideodec GL textures).
GSTREAMER_EXTRA_LIBS := -lssl -lcrypto -lEGL -lGLESv2

include $(GSTREAMER_NDK_BUILD_PATH)/gstreamer-1.0.mk
