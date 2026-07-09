package com.flutter_rust_bridge.xue_hua_video_player;

/**
 * Eagerly initializes {@code AndroidNativeRuntimeBootstrap} from
 * {@link GStreamerInitProvider} before other SDKs can exhaust Bionic pthread keys.
 *
 * <p>Covers FRB handler, {@code xhvp-gst}, GstGL display, and reqwest readiness.
 */
public final class NativeRuntimeWarmup {
    private NativeRuntimeWarmup() {}

    /** Triggers Android native runtime bootstrap on the native side. */
    public static void warmup() {
        nativeWarmupNativeRuntime();
    }

    private static native void nativeWarmupNativeRuntime();
}
