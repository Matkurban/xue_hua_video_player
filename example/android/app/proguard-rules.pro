# App-specific ProGuard rules. Rules for irondash_engine_context and this plugin
# are merged automatically from the xue_hua_video_player AAR (consumerProguardFiles).

-keep class com.flutter_rust_bridge.xue_hua_video_player.IrondashSurfaceProducerCallback { *; }
-keep class org.freedesktop.gstreamer.** { *; }
