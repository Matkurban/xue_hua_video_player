import 'package:flutter/foundation.dart';

/// How the Flutter side embeds GStreamer video for the current platform.
enum VideoSurfaceKind {
  /// Flutter external texture (all supported platforms).
  texture,

  /// Unsupported target platform.
  unsupported,
}

/// Internal identity for routing a player into the correct Dart surface module.
///
/// Not exported from the public package API; integrators use [playerId] on
/// [XueHuaPlayerController] and [XueHuaVideoView].
@immutable
class VideoSurfaceHandle {
  const VideoSurfaceHandle({required this.playerId, required this.kind});

  final int playerId;
  final VideoSurfaceKind kind;

  factory VideoSurfaceHandle.fromPlayerId(int playerId) {
    return VideoSurfaceHandle(
      playerId: playerId,
      kind: _kindForPlatform(defaultTargetPlatform),
    );
  }

  static VideoSurfaceKind kindForPlatform([TargetPlatform? platform]) {
    return _kindForPlatform(platform ?? defaultTargetPlatform);
  }

  static VideoSurfaceKind _kindForPlatform(TargetPlatform platform) {
    return switch (platform) {
      TargetPlatform.iOS ||
      TargetPlatform.macOS ||
      TargetPlatform.windows ||
      TargetPlatform.linux ||
      TargetPlatform.android => VideoSurfaceKind.texture,
      _ => VideoSurfaceKind.unsupported,
    };
  }
}
