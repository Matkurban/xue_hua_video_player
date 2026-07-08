import 'package:flutter/foundation.dart';

/// Flutter 侧嵌入 GStreamer 视频的方式 / How the Flutter side embeds GStreamer video.
enum VideoSurfaceKind {
  /// Flutter 外部 Texture（所有支持的平台）/ Flutter external texture on all supported platforms.
  texture,

  /// 不支持的目标平台 / Unsupported target platform.
  unsupported,
}

/// 将 player 路由至正确 Dart 表面模块的内部标识 / Internal identity for routing a player into the correct Dart surface module.
///
/// 不对外导出；集成方通过 [XueHuaPlayerController] 的 [XueHuaPlayerController.playerId] 与 [XueHuaVideoView] 使用。
/// Not exported publicly; integrators use [XueHuaPlayerController.playerId] and [XueHuaVideoView].
@immutable
class VideoSurfaceHandle {
  /// 创建表面句柄 / Creates a surface handle.
  const VideoSurfaceHandle({required this.playerId, required this.kind});

  /// 原生播放器 ID / Native player id.
  final int playerId;

  /// 当前平台的表面类型 / Surface kind for the current platform.
  final VideoSurfaceKind kind;

  /// 从 [playerId] 与当前 [defaultTargetPlatform] 构造 / Builds from [playerId] and [defaultTargetPlatform].
  factory VideoSurfaceHandle.fromPlayerId(int playerId) {
    return VideoSurfaceHandle(
      playerId: playerId,
      kind: _kindForPlatform(defaultTargetPlatform),
    );
  }

  /// 给定 [platform] 的表面类型 / Surface kind for an optional [platform].
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
