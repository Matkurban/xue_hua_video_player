#include "xhvp_player.h"

#if defined(__ANDROID__)

#include <android/log.h>
#include <android/native_window_jni.h>
#include <jni.h>
#include <stdint.h>

#define LOG_TAG "XhvpNative"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, LOG_TAG, __VA_ARGS__)

static JavaVM *g_vm = NULL;

static void xhvp_android_bind_surface(JNIEnv *env, jlong player_id,
                                      jobject surface, jint width,
                                      jint height) {
  if (surface == NULL || player_id == 0) {
    return;
  }
  ANativeWindow *window = ANativeWindow_fromSurface(env, surface);
  if (!window) {
    LOGE("ANativeWindow_fromSurface failed");
    return;
  }
  int w = width;
  int h = height;
  if (w <= 0) {
    w = ANativeWindow_getWidth(window);
  }
  if (h <= 0) {
    h = ANativeWindow_getHeight(window);
  }
  xhvp_player_notify_android_surface(player_id, (int64_t)(intptr_t)window, w,
                                     h);
}

JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM *vm, void *reserved) {
  (void)reserved;
  g_vm = vm;
  xhvp_init();
  LOGI("JNI_OnLoad: xhvp_init done");
  return JNI_VERSION_1_6;
}

JNIEXPORT void JNICALL
Java_xue_1hua_video_1player_NativeRuntimeWarmup_nativeWarmupNativeRuntime(
    JNIEnv *env, jclass clazz) {
  (void)env;
  (void)clazz;
  xhvp_init();
  LOGI("NativeRuntimeWarmup");
}

JNIEXPORT void JNICALL
Java_xue_1hua_video_1player_AndroidSurfaceBridge_nativeOnSurfaceCreated(
    JNIEnv *env, jclass clazz, jlong player_id, jobject surface) {
  (void)clazz;
  xhvp_android_bind_surface(env, player_id, surface, 0, 0);
}

JNIEXPORT void JNICALL
Java_xue_1hua_video_1player_AndroidSurfaceBridge_nativeOnSurfaceChanged(
    JNIEnv *env, jclass clazz, jlong player_id, jobject surface, jint width,
    jint height) {
  (void)clazz;
  xhvp_android_bind_surface(env, player_id, surface, width, height);
}

JNIEXPORT void JNICALL
Java_xue_1hua_video_1player_AndroidSurfaceBridge_nativeOnSurfaceDestroyed(
    JNIEnv *env, jclass clazz, jlong player_id) {
  (void)env;
  (void)clazz;
  xhvp_player_clear_android_surface(player_id);
}

/* No-op stubs: asset bytes are loaded in Dart; context/plugin binds unused by C core. */
JNIEXPORT void JNICALL
Java_xue_1hua_video_1player_FlutterAssetHelper_nativeBindAssetHelperClass(
    JNIEnv *env, jclass clazz) {
  (void)env;
  (void)clazz;
}

JNIEXPORT void JNICALL
Java_xue_1hua_video_1player_NativeAndroidContext_nativeInitAndroidContext(
    JNIEnv *env, jclass clazz, jobject context) {
  (void)env;
  (void)clazz;
  (void)context;
}

JNIEXPORT void JNICALL
Java_xue_1hua_video_1player_XueHuaVideoPlayerPlugin_nativeBindPluginClass(
    JNIEnv *env, jclass clazz) {
  (void)env;
  (void)clazz;
}

#endif /* __ANDROID__ */
