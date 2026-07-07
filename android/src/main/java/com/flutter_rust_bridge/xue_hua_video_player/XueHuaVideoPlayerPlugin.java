package com.flutter_rust_bridge.xue_hua_video_player;

import android.content.Context;

import androidx.annotation.NonNull;

import io.flutter.FlutterInjector;
import io.flutter.embedding.engine.loader.FlutterLoader;
import io.flutter.embedding.engine.plugins.FlutterPlugin;

/** Registers the GStreamer video Platform View factory. */
public class XueHuaVideoPlayerPlugin implements FlutterPlugin {
    public static final String VIEW_TYPE = "xue_hua_video_player/view";

    static {
        // Must load before Dart FRB calls DynamicLibrary.open — otherwise JNI
        // cannot resolve XueHuaVideoPlatformView native methods on Android.
        System.loadLibrary("xue_hua_video_player");
    }

    @Override
    public void onAttachedToEngine(@NonNull FlutterPluginBinding binding) {
        Context context = binding.getApplicationContext();
        FlutterAssetHelper.init(context);
        FlutterLoader loader = FlutterInjector.instance().flutterLoader();
        try {
            loader.ensureInitializationComplete(context, null);
        } catch (IllegalStateException e) {
            loader.startInitialization(context);
            loader.ensureInitializationComplete(context, null);
        }
        binding
            .getPlatformViewRegistry()
            .registerViewFactory(
                VIEW_TYPE,
                new XueHuaVideoViewFactory(binding.getBinaryMessenger())
            );
    }

    @Override
    public void onDetachedFromEngine(@NonNull FlutterPluginBinding binding) {}
}
