package com.flutter_rust_bridge.xue_hua_video_player;

import android.view.Surface;

import androidx.annotation.NonNull;

import io.flutter.view.TextureRegistry;

/**
 * Feeds a Flutter {@link TextureRegistry.SurfaceProducer} surface into GStreamer
 * {@code glimagesink} via the Rust VideoOverlay path.
 */
final class XueHuaVideoTexture implements TextureRegistry.SurfaceProducer.Callback {
    private final long playerId;
    private final TextureRegistry.SurfaceProducer surfaceProducer;

    XueHuaVideoTexture(long playerId, TextureRegistry textureRegistry) {
        this.playerId = playerId;
        this.surfaceProducer = textureRegistry.createSurfaceProducer();
        this.surfaceProducer.setCallback(this);
        bindSurfaceIfAvailable();
    }

    long textureId() {
        return surfaceProducer.id();
    }

    void dispose() {
        surfaceProducer.setCallback(null);
        AndroidSurfaceBridge.onSurfaceDestroyed(playerId);
        surfaceProducer.release();
    }

    @Override
    public void onSurfaceAvailable() {
        bindSurfaceIfAvailable();
    }

    @Override
    public void onSurfaceCleanup() {
        AndroidSurfaceBridge.onSurfaceDestroyed(playerId);
    }

    private void bindSurfaceIfAvailable() {
        Surface surface = surfaceProducer.getSurface();
        if (surface == null || !surface.isValid()) {
            return;
        }
        int width = surfaceProducer.getWidth();
        int height = surfaceProducer.getHeight();
        if (width <= 0 || height <= 0) {
            AndroidSurfaceBridge.onSurfaceCreated(playerId, surface);
            return;
        }
        AndroidSurfaceBridge.onSurfaceChanged(playerId, surface, width, height);
    }
}
