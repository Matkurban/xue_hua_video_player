package com.flutter_rust_bridge.xue_hua_video_player;

import android.content.Context;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;

import androidx.annotation.NonNull;

import io.flutter.plugin.platform.PlatformView;

/**
 * GStreamer Android tutorial 3 style drawing surface: passes the native window
 * handle to Rust so {@code glimagesink} can render via VideoOverlay.
 */
public class XueHuaVideoPlatformView implements PlatformView, SurfaceHolder.Callback {
    private final SurfaceView surfaceView;
    private final long playerId;

    public XueHuaVideoPlatformView(Context context, long playerId) {
        this.playerId = playerId;
        this.surfaceView = new SurfaceView(context);
        this.surfaceView.getHolder().addCallback(this);
    }

    @NonNull
    @Override
    public View getView() {
        return surfaceView;
    }

    @Override
    public void dispose() {
        surfaceView.getHolder().removeCallback(this);
        nativeOnSurfaceDestroyed(playerId);
    }

    @Override
    public void surfaceCreated(@NonNull SurfaceHolder holder) {
        Surface surface = holder.getSurface();
        if (surface != null) {
            nativeOnSurfaceCreated(playerId, surface);
        }
    }

    @Override
    public void surfaceChanged(
        @NonNull SurfaceHolder holder,
        int format,
        int width,
        int height
    ) {
        Surface surface = holder.getSurface();
        if (surface != null) {
            nativeOnSurfaceChanged(playerId, surface, width, height);
        }
    }

    @Override
    public void surfaceDestroyed(@NonNull SurfaceHolder holder) {
        nativeOnSurfaceDestroyed(playerId);
    }

    private static native void nativeOnSurfaceCreated(long playerId, Surface surface);

    private static native void nativeOnSurfaceChanged(
        long playerId,
        Surface surface,
        int width,
        int height
    );

    private static native void nativeOnSurfaceDestroyed(long playerId);
}
