import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import 'texture_surface.dart';
import 'video_surface_handle.dart';

/// Routes [handle] to the Flutter external [Texture] surface for its platform.
Widget buildVideoSurface(VideoSurfaceHandle handle) {
  return switch (handle.kind) {
    VideoSurfaceKind.texture => TextureVideoSurface(handle: handle),
    VideoSurfaceKind.unsupported => ColoredBox(
      color: Colors.black,
      child: Center(
        child: Text('Video not supported on $defaultTargetPlatform'),
      ),
    ),
  };
}
