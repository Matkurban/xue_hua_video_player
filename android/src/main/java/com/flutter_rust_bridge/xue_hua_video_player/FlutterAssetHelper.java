package com.flutter_rust_bridge.xue_hua_video_player;

import android.content.Context;
import android.content.res.AssetFileDescriptor;
import android.content.res.AssetManager;

import androidx.annotation.Keep;

import io.flutter.FlutterInjector;
import io.flutter.embedding.engine.loader.FlutterLoader;

/**
 * Opens Flutter bundle assets for Rust AppSrc playback via detached file descriptors.
 */
@Keep
public final class FlutterAssetHelper {
  private static volatile Context appContext;

  private FlutterAssetHelper() {}

  /** Called during process startup from {@link GStreamerInitProvider}. */
  public static void init(Context context) {
    appContext = context.getApplicationContext();
  }

  /**
   * Returns {@code [fd, startOffset, length]} or {@code [-1, 0, 0]} when unavailable.
   */
  public static long[] openAssetFd(String assetKey) {
    try {
      Context context = appContext;
      if (context == null) {
        return new long[] { -1, 0, 0 };
      }
      FlutterLoader loader = FlutterInjector.instance().flutterLoader();
      String lookupKey = loader.getLookupKeyForAsset(assetKey);
      AssetManager assets = context.getAssets();
      AssetFileDescriptor afd = assets.openFd(lookupKey);
      long start = afd.getStartOffset();
      long length = afd.getLength();
      int fd = afd.getParcelFileDescriptor().detachFd();
      afd.close();
      return new long[] { fd, start, length };
    } catch (Exception e) {
      return new long[] { -1, 0, 0 };
    }
  }
}
