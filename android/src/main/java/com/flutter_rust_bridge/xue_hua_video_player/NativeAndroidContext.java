package com.flutter_rust_bridge.xue_hua_video_player;

import android.content.Context;

/**
 * Initializes {@code ndk-context} in Rust with the application {@link Context}.
 *
 * <p>Called from {@link GStreamerInitProvider} after {@code GStreamer.init(context)} so Rust
 * can access the JVM and Android context without jni-rs TLS attach guards.
 */
public final class NativeAndroidContext {
    private NativeAndroidContext() {}

  /** Binds the process {@link Context} for Rust {@code ndk_context::initialize_android_context}. */
    public static void init(Context context) {
        nativeInitAndroidContext(context);
    }

    private static native void nativeInitAndroidContext(Context context);
}
