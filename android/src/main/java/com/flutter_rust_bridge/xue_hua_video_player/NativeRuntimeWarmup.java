package com.flutter_rust_bridge.xue_hua_video_player;

/**
 * Eagerly initializes the Rust FRB handler and {@code xhvp-gst} thread from
 * {@link GStreamerInitProvider} before other SDKs can exhaust Bionic pthread keys.
 */
public final class NativeRuntimeWarmup {
    private NativeRuntimeWarmup() {}

    /** Triggers FRB handler + Gst runtime initialization on the native side. */
    public static void warmup() {
        nativeWarmupNativeRuntime();
    }

    private static native void nativeWarmupNativeRuntime();
}
