package xue_hua.video_player;

import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.util.DisplayMetrics;
import android.view.Surface;

import androidx.annotation.Nullable;

import io.flutter.view.TextureRegistry;

/**
 * Feeds a Flutter {@link TextureRegistry.SurfaceProducer} surface into GStreamer
 * {@code glimagesink} via VideoOverlay.
 *
 * <p>SurfaceProducer lifecycle is the source of truth: bind/unbind {@code ANativeWindow}
 * only from {@link #onSurfaceAvailable} / {@link #onSurfaceCleanup} / {@link #dispose}.
 * Do not drive {@link TextureRegistry.SurfaceProducer#setSize} from decoded video caps.
 * Layout-driven {@link #syncSize} improves resolution; an eager DisplayMetrics 16:9
 * size avoids staying on the default 1×1 buffer until Dart layout sync runs.
 *
 * <p>{@link TextureRegistry.SurfaceProducer#setSize} destroys the old buffer. Unbind
 * VideoOverlay <em>synchronously</em> before {@code setSize} so glimagesink cannot call
 * {@code eglCreateWindowSurface} on a destroyed Surface (FORTIFY destroyed-mutex abort).
 * While resizing, ignore {@link #onSurfaceCleanup}. On cleanup, always clear then
 * post a rebind so Flutter can recreate the ImageReader ({@code getSurface}).
 */
final class XueHuaVideoTexture implements TextureRegistry.SurfaceProducer.Callback {
    /**
     * Test hook: when non-null, run posted rebind tasks through this instead of
     * {@link Handler} (JVM unit tests have no main looper).
     */
    interface RebindScheduler {
        void post(Runnable task);
    }

    static volatile RebindScheduler rebindScheduler;

    private final long playerId;
    private final TextureRegistry.SurfaceProducer surfaceProducer;
    /** True while {@link #syncSize} owns the resize; skip cleanup→destroy race. */
    private boolean resizing;
    private boolean disposed;
    /** Last Surface passed to native; skip redundant ANativeWindow_fromSurface. */
    private Surface boundSurface;
    private int boundWidth;
    private int boundHeight;

    XueHuaVideoTexture(long playerId, TextureRegistry textureRegistry) {
        this(playerId, textureRegistry, null);
    }

    XueHuaVideoTexture(
        long playerId,
        TextureRegistry textureRegistry,
        @Nullable Context context
    ) {
        this.playerId = playerId;
        this.surfaceProducer = textureRegistry.createSurfaceProducer();
        this.surfaceProducer.setCallback(this);
        // Bind immediately: GStreamer Android requires a native window before
        // playbin can preroll (audio included). Prefer a screen-fitted size so
        // we are not stuck on the default 1×1 until Dart syncTextureSize runs.
        if (context != null) {
            eagerSyncFromDisplay(context);
        } else {
            bindSurfaceIfAvailable();
        }
    }

    long textureId() {
        return surfaceProducer.id();
    }

    /**
     * Resize the SurfaceProducer buffer from Flutter layout (physical pixels), then
     * re-bind so GStreamer gets an updated window / render rectangle.
     */
    void syncSize(int width, int height) {
        if (disposed) {
            return;
        }
        if (width <= 1 || height <= 1) {
            return;
        }
        if (surfaceProducer.getWidth() == width && surfaceProducer.getHeight() == height) {
            // Still re-bind if native lost the window after a spurious cleanup.
            bindSurfaceIfAvailable();
            return;
        }
        resizing = true;
        try {
            // Unbind GST from the current window BEFORE setSize destroys the Surface.
            clearBoundSurface();
            surfaceProducer.setSize(width, height);
            bindSurfaceIfAvailable();
        } finally {
            resizing = false;
        }
    }

    void dispose() {
        disposed = true;
        surfaceProducer.setCallback(null);
        clearBoundSurface();
        surfaceProducer.release();
    }

    @Override
    public void onSurfaceAvailable() {
        bindSurfaceIfAvailable();
    }

    @Override
    public void onSurfaceCleanup() {
        if (disposed || resizing) {
            // setSize invalidated the old buffer; syncSize will bind the new one.
            return;
        }
        // Always clear: Flutter may be about to close this ImageReader (trim /
        // release). Leaving GST bound causes abandoned BufferQueue / EGL_BAD_SURFACE.
        clearBoundSurface();
        scheduleRebind();
    }

    /** Contain-fit 16:9 into the display in physical pixels, then bind. */
    private void eagerSyncFromDisplay(Context context) {
        DisplayMetrics metrics = context.getResources().getDisplayMetrics();
        int[] size = containFit16x9(metrics.widthPixels, metrics.heightPixels);
        if (size == null) {
            bindSurfaceIfAvailable();
            return;
        }
        syncSize(size[0], size[1]);
    }

    /**
     * Contain-fit 16:9 into [screenW]×[screenH]. Returns null if inputs are unusable.
     */
    @Nullable
    static int[] containFit16x9(int screenW, int screenH) {
        if (screenW <= 1 || screenH <= 1) {
            return null;
        }
        final double ratio = 16.0 / 9.0;
        final double screenAspect = (double) screenW / (double) screenH;
        final int w;
        final int h;
        if (screenAspect > ratio) {
            h = screenH;
            w = (int) Math.round(h * ratio);
        } else {
            w = screenW;
            h = (int) Math.round(w / ratio);
        }
        if (w <= 1 || h <= 1) {
            return null;
        }
        return new int[] {w, h};
    }

    private void scheduleRebind() {
        final Runnable task =
            () -> {
                if (!disposed) {
                    bindSurfaceIfAvailable();
                }
            };
        RebindScheduler scheduler = rebindScheduler;
        if (scheduler != null) {
            scheduler.post(task);
            return;
        }
        new Handler(Looper.getMainLooper()).post(task);
    }

    private void clearBoundSurface() {
        boundSurface = null;
        boundWidth = 0;
        boundHeight = 0;
        AndroidSurfaceBridge.onSurfaceDestroyed(playerId);
    }

    private void bindSurfaceIfAvailable() {
        if (disposed) {
            return;
        }
        Surface surface = surfaceProducer.getSurface();
        if (surface == null || !surface.isValid()) {
            return;
        }
        // Pass the SurfaceProducer buffer size (physical pixels), not decoded frame size.
        int width = surfaceProducer.getWidth();
        int height = surfaceProducer.getHeight();
        if (surface == boundSurface && width == boundWidth && height == boundHeight) {
            // Same Surface instance: skip ANativeWindow_fromSurface (new pointer each
            // call would force native clear+re-apply and extra EGL churn).
            return;
        }
        boundSurface = surface;
        boundWidth = width;
        boundHeight = height;
        AndroidSurfaceBridge.onSurfaceChanged(playerId, surface, width, height);
    }
}
