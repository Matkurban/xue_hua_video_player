package com.example.xue_hua_video_player_example

import io.flutter.embedding.android.FlutterActivity

// No GStreamer initialization is needed here: the plugin bundles the umbrella
// libgstreamer_android.so and auto-initializes the GStreamer Android runtime at
// process startup via GStreamerInitProvider (a ContentProvider that runs
// System.loadLibrary("gstreamer_android") + GStreamer.init(context) so the
// androidmedia MediaCodec decoders can register). See the plugin's
// android/src/main/java/.../GStreamerInitProvider.java.
class MainActivity : FlutterActivity()
