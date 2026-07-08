package com.flutter_rust_bridge.xue_hua_video_player;

/** JNI entry points for {@link XueHuaVideoTexture} (SurfaceProducer → GStreamer). */
final class AndroidSurfaceBridge {
    private AndroidSurfaceBridge() {}

    static void onSurfaceCreated(long playerId, android.view.Surface surface) {
        nativeOnSurfaceCreated(playerId, surface);
    }

    static void onSurfaceChanged(
        long playerId,
        android.view.Surface surface,
        int width,
        int height
    ) {
        nativeOnSurfaceChanged(playerId, surface, width, height);
    }

    static void onSurfaceDestroyed(long playerId) {
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
