package com.flutter_rust_bridge.xue_hua_video_player;

import android.content.Context;

import androidx.annotation.NonNull;

import io.flutter.FlutterInjector;
import io.flutter.embedding.engine.loader.FlutterLoader;
import io.flutter.embedding.engine.plugins.FlutterPlugin;
import io.flutter.plugin.common.MethodCall;
import io.flutter.plugin.common.MethodChannel;
import io.flutter.view.TextureRegistry;

import java.util.HashMap;
import java.util.Map;

/** Registers GStreamer video textures via Flutter {@link TextureRegistry.SurfaceProducer}. */
public class XueHuaVideoPlayerPlugin implements FlutterPlugin, MethodChannel.MethodCallHandler {
    public static final String TEXTURE_CHANNEL_NAME = "xue_hua_video_player/texture";

    static {
        System.loadLibrary("xue_hua_video_player");
    }

    private MethodChannel textureChannel;
    private TextureRegistry textureRegistry;
    private final Map<Long, XueHuaVideoTexture> videoTextures = new HashMap<>();

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

        textureRegistry = binding.getTextureRegistry();
        textureChannel = new MethodChannel(
            binding.getBinaryMessenger(),
            TEXTURE_CHANNEL_NAME
        );
        textureChannel.setMethodCallHandler(this);
    }

    @Override
    public void onMethodCall(@NonNull MethodCall call, @NonNull MethodChannel.Result result) {
        Long playerId = call.argument("playerId");
        if (playerId == null || playerId == 0L) {
            result.error("invalid_args", "playerId required", null);
            return;
        }
        switch (call.method) {
            case "createTexture":
                synchronized (videoTextures) {
                    XueHuaVideoTexture existing = videoTextures.get(playerId);
                    if (existing != null) {
                        result.success(existing.textureId());
                        return;
                    }
                    XueHuaVideoTexture texture =
                        new XueHuaVideoTexture(playerId, textureRegistry);
                    videoTextures.put(playerId, texture);
                    result.success(texture.textureId());
                }
                break;
            case "disposeTexture":
                synchronized (videoTextures) {
                    XueHuaVideoTexture texture = videoTextures.remove(playerId);
                    if (texture != null) {
                        texture.dispose();
                    }
                }
                result.success(null);
                break;
            default:
                result.notImplemented();
                break;
        }
    }

    @Override
    public void onDetachedFromEngine(@NonNull FlutterPluginBinding binding) {
        if (textureChannel != null) {
            textureChannel.setMethodCallHandler(null);
            textureChannel = null;
        }
        synchronized (videoTextures) {
            for (XueHuaVideoTexture texture : videoTextures.values()) {
                texture.dispose();
            }
            videoTextures.clear();
        }
        textureRegistry = null;
    }
}
