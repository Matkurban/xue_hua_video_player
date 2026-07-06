# JNI natives for Platform View surface callbacks
-keep class com.flutter_rust_bridge.xue_hua_video_player.XueHuaVideoPlatformView { *; }

# Flutter texture registry types (if referenced by embedding)
-keep interface io.flutter.view.TextureRegistry { *; }
-keep class io.flutter.view.TextureRegistry$* { *; }
