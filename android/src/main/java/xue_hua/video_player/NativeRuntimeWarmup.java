package xue_hua.video_player;

/**
 * Eagerly initializes the C player runtime from {@link GStreamerInitProvider}
 * after {@code GStreamer.init(Context)} and before other SDKs can exhaust Bionic
 * pthread keys.
 */
public final class NativeRuntimeWarmup {
    private NativeRuntimeWarmup() {}

    /**
     * Triggers {@code xhvp_init} on the native side. Must run after
     * {@code GStreamer.init} so MediaCodec discovery has an application Context.
     */
    public static void warmup() {
        nativeWarmupNativeRuntime();
    }

    private static native void nativeWarmupNativeRuntime();
}
