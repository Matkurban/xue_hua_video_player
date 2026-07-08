package com.flutter_rust_bridge.xue_hua_video_player;

import android.content.Context;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

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

    private static native void nativeBindPluginClass();

    @Nullable
    private static volatile XueHuaVideoPlayerPlugin instance;

    private MethodChannel textureChannel;
    private TextureRegistry textureRegistry;
    private final Map<Long, XueHuaVideoTexture> videoTextures = new HashMap<>();

    /** Called from Rust/JNI when video caps negotiate a new content size. */
    public static void setTextureContentSizeSync(long playerId, int width, int height) {
        XueHuaVideoPlayerPlugin plugin = instance;
        if (plugin == null) {
            return;
        }
        plugin.applyTextureContentSize(playerId, width, height);
    }

    private void applyTextureContentSize(long playerId, int width, int height) {
        XueHuaVideoTexture texture;
        synchronized (videoTextures) {
            texture = videoTextures.get(playerId);
        }
        if (texture != null) {
            texture.setContentSizeSync(width, height);
        }
    }

    @Override
    public void onAttachedToEngine(@NonNull FlutterPluginBinding binding) {
        instance = this;
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
        nativeBindPluginClass();
    }

    @Override
    public void onMethodCall(@NonNull MethodCall call, @NonNull MethodChannel.Result result) {
        long playerId = playerIdFromCall(call);
        if (playerId == 0L) {
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

    /** StandardMessageCodec may deliver small ints as {@link Integer}, not {@link Long}. */
    private static long playerIdFromCall(@NonNull MethodCall call) {
        Object raw = call.argument("playerId");
        if (raw instanceof Number) {
            return ((Number) raw).longValue();
        }
        return 0L;
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
        instance = null;
    }
}
