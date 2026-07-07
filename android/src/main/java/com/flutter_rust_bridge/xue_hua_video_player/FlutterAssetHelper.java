package com.flutter_rust_bridge.xue_hua_video_player;

import android.content.Context;
import android.content.res.AssetFileDescriptor;
import android.content.res.AssetManager;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;

import androidx.annotation.Keep;

import io.flutter.FlutterInjector;
import io.flutter.embedding.engine.loader.FlutterLoader;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Opens Flutter bundle assets for Rust AppSrc playback via detached file descriptors.
 */
@Keep
public final class FlutterAssetHelper {
  private static final String TAG = "FlutterAssetHelper";
  private static volatile Context appContext;

  private FlutterAssetHelper() {}

  /** Called during process startup from {@link GStreamerInitProvider}. */
  public static void init(Context context) {
    appContext = context.getApplicationContext();
    nativeBindAssetHelperClass();
  }

  private static native void nativeBindAssetHelperClass();

  /**
   * Returns {@code [fd, startOffset, length]} or {@code [-1, 0, 0]} when unavailable.
   *
   * <p>Marshals to the main thread because {@link FlutterLoader#ensureInitializationComplete}
   * must run on the main looper.
   */
  public static long[] openAssetFd(String assetKey) {
    if (Looper.myLooper() == Looper.getMainLooper()) {
      return openAssetFdOnMain(assetKey);
    }
    final AtomicReference<long[]> holder =
        new AtomicReference<>(new long[] { -1, 0, 0 });
    final CountDownLatch latch = new CountDownLatch(1);
    new Handler(Looper.getMainLooper())
        .post(
            () -> {
              try {
                holder.set(openAssetFdOnMain(assetKey));
              } finally {
                latch.countDown();
              }
            });
    try {
      if (!latch.await(5, TimeUnit.SECONDS)) {
        Log.e(TAG, "openAssetFd timed out for key=" + assetKey);
      }
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      Log.e(TAG, "openAssetFd interrupted for key=" + assetKey, e);
    }
    return holder.get();
  }

  private static long[] openAssetFdOnMain(String assetKey) {
    try {
      Context context = appContext;
      if (context == null) {
        Log.e(TAG, "openAssetFd: appContext is null for key=" + assetKey);
        return new long[] { -1, 0, 0 };
      }
      FlutterLoader loader = FlutterInjector.instance().flutterLoader();
      ensureFlutterLoaderReady(loader, context);
      String lookupKey = loader.getLookupKeyForAsset(assetKey);
      AssetManager assets = context.getAssets();
      AssetFileDescriptor afd = assets.openFd(lookupKey);
      long start = afd.getStartOffset();
      long length = afd.getLength();
      int fd = afd.getParcelFileDescriptor().detachFd();
      afd.close();
      Log.i(
          TAG,
          "openAssetFd ok key="
              + assetKey
              + " lookup="
              + lookupKey
              + " len="
              + length);
      return new long[] { fd, start, length };
    } catch (Exception e) {
      Log.e(TAG, "openAssetFd failed key=" + assetKey + ": " + e.getMessage(), e);
      return new long[] { -1, 0, 0 };
    }
  }

  private static void ensureFlutterLoaderReady(FlutterLoader loader, Context context) {
    try {
      loader.ensureInitializationComplete(context, null);
    } catch (IllegalStateException e) {
      loader.startInitialization(context);
      loader.ensureInitializationComplete(context, null);
    }
  }
}
