package com.flutter_rust_bridge.xue_hua_video_player;

import android.content.Context;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

import java.util.Map;

import io.flutter.plugin.common.BinaryMessenger;
import io.flutter.plugin.common.StandardMessageCodec;
import io.flutter.plugin.platform.PlatformView;
import io.flutter.plugin.platform.PlatformViewFactory;

/** Creates [XueHuaVideoPlatformView] instances for each Flutter Platform View. */
public class XueHuaVideoViewFactory extends PlatformViewFactory {
    private final BinaryMessenger messenger;

    public XueHuaVideoViewFactory(BinaryMessenger messenger) {
        super(StandardMessageCodec.INSTANCE);
        this.messenger = messenger;
    }

    @NonNull
    @Override
    public PlatformView create(@NonNull Context context, int viewId, @Nullable Object args) {
        long playerId = 0L;
        if (args instanceof Map) {
            Object raw = ((Map<?, ?>) args).get("playerId");
            if (raw instanceof Number) {
                playerId = ((Number) raw).longValue();
            }
        }
        return new XueHuaVideoPlatformView(context, playerId);
    }
}
