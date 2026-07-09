package com.flutter_rust_bridge.xue_hua_video_player;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.net.Uri;
import android.util.Log;

import org.freedesktop.gstreamer.GStreamer;

/**
 * Auto-initializes the GStreamer Android runtime before any Dart/Rust code runs.
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
 */
public class GStreamerInitProvider extends ContentProvider {
    private static final String TAG = "XueHuaGStreamerInit";

    @Override
    public boolean onCreate() {
        try {
            System.loadLibrary("gstreamer_android");
            // Load before Dart FRB dlopen so Platform View JNI symbols resolve.
            System.loadLibrary("xue_hua_video_player");
            GStreamer.init(getContext());
            FlutterAssetHelper.init(getContext());
            NativeAndroidContext.init(getContext());
            NativeRuntimeWarmup.warmup();
            Log.i(TAG, "GStreamer Android runtime initialized");
        } catch (Throwable t) {
            Log.e(TAG, "Failed to initialize GStreamer Android runtime", t);
        }
        return true;
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
