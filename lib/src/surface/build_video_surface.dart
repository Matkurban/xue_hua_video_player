import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import 'texture_surface.dart';
import 'video_surface_handle.dart';

/// 将 [handle] 路由至平台对应的 Flutter 外部 [Texture] 表面 / Routes [handle] to the platform Flutter external [Texture] surface.
///
/// # 参数 / Parameters
/// - `handle` — [VideoSurfaceHandle] 含 playerId 与 [VideoSurfaceKind] / player id and surface kind
///
/// # 返回值 / Returns
/// - [VideoSurfaceKind.texture] → [TextureVideoSurface]
/// - [VideoSurfaceKind.unsupported] → 错误占位 widget / error placeholder widget
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
