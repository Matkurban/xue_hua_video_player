package com.example.xue_hua_video_player_example

import android.os.Bundle
import io.flutter.embedding.android.FlutterActivity

class MainActivity : FlutterActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // GStreamer on Android must be initialized once, on the main thread,
        // before any pipeline is created. `GStreamer.init` extracts bundled
        // certificates/plugins and registers the statically-linked plugins.
        //
        // Requires the GStreamer Android SDK's Java sources (the
        // `org.freedesktop.gstreamer.GStreamer` class) to be on the classpath.
        // See the plugin README ("Android") for setup. The reflective call keeps
        // the example compiling even before the SDK is wired in.
        try {
            val clazz = Class.forName("org.freedesktop.gstreamer.GStreamer")
            clazz.getMethod("init", android.content.Context::class.java)
                .invoke(null, this)
        } catch (e: ClassNotFoundException) {
            // GStreamer Android SDK not yet integrated; see README.
        } catch (e: Exception) {
            android.util.Log.e("xue_hua_video_player", "GStreamer.init failed", e)
        }
    }
}
