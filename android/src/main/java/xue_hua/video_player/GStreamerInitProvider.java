package xue_hua.video_player;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.net.Uri;
import android.util.Log;

import org.freedesktop.gstreamer.GStreamer;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Auto-initializes the GStreamer Android runtime before any Dart FFI code runs.
 *
 * <p>The {@code libgstreamer_android.so} umbrella library is built at compile time
 * and packaged into the plugin AAR. It must be loaded through
 * {@link System#loadLibrary} here (not Dart FFI {@code dlopen}) so the library's
 * {@code JNI_OnLoad} runs, the JavaVM is captured, and the {@code androidmedia}
 * (MediaCodec) decoders register. Without this, playback fails with
 * "not-linked" / "No streams to output".
 *
 * <p>{@link GStreamer#init} sets the application {@code Context}/{@code ClassLoader}
 * required for MediaCodec codec discovery. A {@link ContentProvider} is used
 * because its {@link #onCreate()} runs during process startup - before
 * {@code Application.onCreate} and long before the Flutter engine.
 *
 * <p>Order is load libraries → {@code GStreamer.init(context)} on the main
 * thread, then {@code xhvp_init} on a <em>background</em> thread so process
 * startup is not blocked on {@code gst_init}. Dart {@code initialize()} shares
 * the same native start gate and waits until warmup completes.
 */
public class GStreamerInitProvider extends ContentProvider {
    private static final String TAG = "XueHuaGStreamerInit";

    private static final CountDownLatch WARMUP_DONE = new CountDownLatch(1);
    private static final AtomicBoolean WARMUP_STARTED = new AtomicBoolean(false);

    /**
     * Blocks until background {@code xhvp_init} finishes (or timeout). Safe to
     * call from Dart/native paths that need a ready runtime.
     */
    public static boolean awaitWarmup(long timeoutMs) {
        try {
            return WARMUP_DONE.await(timeoutMs, TimeUnit.MILLISECONDS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return false;
        }
    }

    @Override
    public boolean onCreate() {
        final long totalNs = System.nanoTime();
        try {
            long t = System.nanoTime();
            System.loadLibrary("gstreamer_android");
            logMs("load_gstreamer_android", t);

            // Load C player before Dart FFI so JNI surface symbols resolve.
            // JNI_OnLoad only captures JavaVM; xhvp_init runs in warmup below.
            t = System.nanoTime();
            System.loadLibrary("xue_hua_video_player");
            logMs("load_xue_hua_video_player", t);

            t = System.nanoTime();
            GStreamer.init(getContext());
            logMs("gstreamer_init", t);

            FlutterAssetHelper.init(getContext());
            NativeAndroidContext.init(getContext());

            startBackgroundWarmup();

            logMs("provider_total", totalNs);
            Log.i(TAG, "GStreamer Android libs ready; native warmup async");
        } catch (Throwable t) {
            Log.e(TAG, "Failed to initialize GStreamer Android runtime", t);
            WARMUP_DONE.countDown();
        }
        return true;
    }

    private static void startBackgroundWarmup() {
        if (!WARMUP_STARTED.compareAndSet(false, true)) {
            return;
        }
        final Thread thread =
                new Thread(
                        () -> {
                            final long t = System.nanoTime();
                            try {
                                NativeRuntimeWarmup.warmup();
                                logMs("native_warmup", t);
                            } catch (Throwable e) {
                                Log.e(TAG, "Background native warmup failed", e);
                            } finally {
                                WARMUP_DONE.countDown();
                            }
                        },
                        "xhvp-native-warmup");
        thread.setDaemon(true);
        thread.start();
    }

    private static void logMs(String phase, long startNs) {
        final long ms = (System.nanoTime() - startNs) / 1_000_000L;
        Log.i(TAG, "[xhvp-init-timing] " + phase + "=" + ms + "ms");
    }

    @Override
    public Cursor query(Uri uri, String[] projection, String selection,
                        String[] selectionArgs, String sortOrder) {
        return null;
    }

    @Override
    public String getType(Uri uri) {
        return null;
    }

    @Override
    public Uri insert(Uri uri, ContentValues values) {
        return null;
    }

    @Override
    public int delete(Uri uri, String selection, String[] selectionArgs) {
        return 0;
    }

    @Override
    public int update(Uri uri, ContentValues values, String selection,
                      String[] selectionArgs) {
        return 0;
    }
}
