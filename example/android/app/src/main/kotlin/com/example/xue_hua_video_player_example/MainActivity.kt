package com.example.xue_hua_video_player_example

import io.flutter.embedding.android.FlutterActivity

// No GStreamer initialization is needed here: the plugin bundles the umbrella
// libgstreamer_android.so and the Rust core registers the statically-linked
// GStreamer plugins itself (gst_init_static_plugins in rust/src/player.rs).
class MainActivity : FlutterActivity()
