package xue_hua.video_player;

import android.view.Surface;

/** JNI entry points for {@link XueHuaVideoTexture} (SurfaceProducer → GStreamer). */
final class AndroidSurfaceBridge {
    private AndroidSurfaceBridge() {}

    /**
     * Optional test hook. When non-null, static methods record via the listener and
     * skip JNI (JVM unit tests cannot load the native library).
     */
    interface Listener {
        void onSurfaceChanged(long playerId, Surface surface, int width, int height);

        void onSurfaceDestroyed(long playerId);
    }

    static volatile Listener listener;

    static void onSurfaceCreated(long playerId, android.view.Surface surface) {
        nativeOnSurfaceCreated(playerId, surface);
    }

    static void onSurfaceChanged(
        long playerId,
        android.view.Surface surface,
        int width,
        int height
    ) {
        Listener l = listener;
        if (l != null) {
            l.onSurfaceChanged(playerId, surface, width, height);
            return;
        }
        nativeOnSurfaceChanged(playerId, surface, width, height);
    }

    static void onSurfaceDestroyed(long playerId) {
        Listener l = listener;
        if (l != null) {
            l.onSurfaceDestroyed(playerId);
            return;
        }
        nativeOnSurfaceDestroyed(playerId);
    }

    private static native void nativeOnSurfaceCreated(long playerId, android.view.Surface surface);

    private static native void nativeOnSurfaceChanged(
        long playerId,
        android.view.Surface surface,
        int width,
        int height
    );

    private static native void nativeOnSurfaceDestroyed(long playerId);
}
