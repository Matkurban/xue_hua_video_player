# irondash_engine_context — JNI-called static methods used by Rust FFI
-keep class dev.irondash.engine_context.** { *; }

# Flutter texture registry (return type of getTextureRegistry)
-keep interface io.flutter.view.TextureRegistry { *; }
-keep class io.flutter.view.TextureRegistry$* { *; }

# GStreamer startup ContentProvider (runs before Flutter engine)
-keep class com.flutter_rust_bridge.xue_hua_video_player.GStreamerInitProvider { *; }
