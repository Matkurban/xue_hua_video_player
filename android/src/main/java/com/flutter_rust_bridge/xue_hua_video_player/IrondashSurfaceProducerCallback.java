package com.flutter_rust_bridge.xue_hua_video_player;

import io.flutter.view.TextureRegistry;

/**
 * Forwards {@link TextureRegistry.SurfaceProducer} lifecycle events to Rust so
 * {@code ANativeWindow} handles are released when Flutter destroys the surface
 * (e.g. app backgrounded) and refreshed when it becomes available again.
 */
public final class IrondashSurfaceProducerCallback
        implements TextureRegistry.SurfaceProducer.Callback {
    private final long textureId;

    public IrondashSurfaceProducerCallback(long textureId) {
        this.textureId = textureId;
    }

    @Override
    public void onSurfaceAvailable() {
        nativeOnSurfaceAvailable(textureId);
    }

    @Override
    public void onSurfaceCleanup() {
        nativeOnSurfaceCleanup(textureId);
    }

    private static native void nativeOnSurfaceAvailable(long textureId);

    private static native void nativeOnSurfaceCleanup(long textureId);
}
