import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import 'desktop_overlay.dart';
import 'mobile_platform_view.dart';
import 'video_surface_handle.dart';

/// Routes [handle] to the correct Dart surface implementation for its [VideoSurfaceKind].
Widget buildVideoSurface(VideoSurfaceHandle handle) {
  return switch (handle.kind) {
    VideoSurfaceKind.platformView => buildMobilePlatformView(handle),
    VideoSurfaceKind.desktopOverlay => DesktopVideoOverlay(handle: handle),
    VideoSurfaceKind.unsupported => ColoredBox(
      color: Colors.black,
      child: Center(
        child: Text('Video not supported on $defaultTargetPlatform'),
      ),
    ),
  };
}
