package xue_hua.video_player;

import android.content.Context;

/**
 * Initializes the Android application {@link Context} for the native player.
 *
 * <p>Called from {@link GStreamerInitProvider} after {@code GStreamer.init(context)}.
 * The C core currently treats this as a no-op stub (assets are loaded via Dart FFI).
 */
public final class NativeAndroidContext {
    private NativeAndroidContext() {}

  /** Binds the process {@link Context} for native Android helpers (stub). */
    public static void init(Context context) {
        nativeInitAndroidContext(context);
    }

    private static native void nativeInitAndroidContext(Context context);
}
