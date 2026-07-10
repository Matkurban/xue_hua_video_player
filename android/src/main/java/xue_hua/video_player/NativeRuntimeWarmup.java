package xue_hua.video_player;

/**
 * Eagerly initializes the C player runtime from {@link GStreamerInitProvider}
 * before other SDKs can exhaust Bionic pthread keys.
 */
public final class NativeRuntimeWarmup {
    private NativeRuntimeWarmup() {}

    /** Triggers {@code xhvp_init} on the native side. */
    public static void warmup() {
        nativeWarmupNativeRuntime();
    }

    private static native void nativeWarmupNativeRuntime();
}
