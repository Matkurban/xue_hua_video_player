package com.flutter_rust_bridge.xue_hua_video_player;

import android.os.Handler;
import android.os.Looper;
import android.view.Surface;

import io.flutter.view.TextureRegistry;

/**
 * Feeds a Flutter {@link TextureRegistry.SurfaceProducer} surface into GStreamer
 * {@code glimagesink} via the Rust VideoOverlay path.
 */
final class XueHuaVideoTexture implements TextureRegistry.SurfaceProducer.Callback {
    private static final Handler MAIN_HANDLER = new Handler(Looper.getMainLooper());

    private final long playerId;
    private final TextureRegistry.SurfaceProducer surfaceProducer;
    private int contentWidth = 0;
    private int contentHeight = 0;

    XueHuaVideoTexture(long playerId, TextureRegistry textureRegistry) {
        this.playerId = playerId;
        this.surfaceProducer = textureRegistry.createSurfaceProducer();
        this.surfaceProducer.setCallback(this);
        // TextureVideoPlayer pattern: bind immediately after setCallback.
        // onSurfaceAvailable() only fires after lifecycle destroy/resume, not on first create.
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

    /**
     * Resizes the ImageReader backing store to the negotiated video resolution
     * (content pixels, not widget layout size) and rebinds the surface to GStreamer.
     */
    void setContentSize(int width, int height) {
        if (width < 2 || height < 2) {
            return;
        }
        Runnable work =
            () -> {
                if (width == contentWidth && height == contentHeight) {
                    bindSurfaceIfAvailable();
                    return;
                }
                contentWidth = width;
                contentHeight = height;
                surfaceProducer.setSize(width, height);
                bindSurfaceIfAvailable();
            };
        if (Looper.myLooper() == Looper.getMainLooper()) {
            work.run();
        } else {
            MAIN_HANDLER.post(work);
        }
    }

    /**
     * Blocks the caller until {@link #setContentSize} finishes on the main thread.
     * Used from Rust/JNI on the GStreamer thread to preserve setSize-before-overlay order.
     */
    void setContentSizeSync(int width, int height) {
        if (width < 2 || height < 2) {
            return;
        }
        if (Looper.myLooper() == Looper.getMainLooper()) {
            setContentSize(width, height);
            return;
        }
        final Object lock = new Object();
        final boolean[] done = {false};
        MAIN_HANDLER.post(
            () -> {
                try {
                    setContentSize(width, height);
                } finally {
                    synchronized (lock) {
                        done[0] = true;
                        lock.notifyAll();
                    }
                }
            });
        synchronized (lock) {
            while (!done[0]) {
                try {
                    lock.wait(3000L);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    return;
                }
            }
        }
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
        AndroidSurfaceBridge.onSurfaceChanged(playerId, surface, 0, 0);
    }
}
