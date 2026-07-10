# Consumer ProGuard / R8 rules for apps that minify with this plugin.

-keep class org.freedesktop.gstreamer.** { *; }

-keep class xue_hua.video_player.GStreamerInitProvider { *; }
-keep class xue_hua.video_player.NativeRuntimeWarmup { *; }
-keep class xue_hua.video_player.NativeAndroidContext { *; }
-keep class xue_hua.video_player.FlutterAssetHelper { *; }
-keep class xue_hua.video_player.XueHuaVideoPlayerPlugin { *; }
-keep class xue_hua.video_player.AndroidSurfaceBridge { *; }
-keep class xue_hua.video_player.XueHuaVideoTexture { *; }
