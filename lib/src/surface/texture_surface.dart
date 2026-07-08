import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'video_surface_handle.dart';

/// 原生 Texture 注册 MethodChannel（各平台插件 `xue_hua_video_player/texture`）/ MethodChannel to the native texture registrar.
const MethodChannel _textureChannel = MethodChannel(
  'xue_hua_video_player/texture',
);

/// 通过 Flutter 外部 [Texture] 渲染 player 视频 / Renders a player's video through a Flutter external [Texture].
///
/// 按 [VideoSurfaceHandle.kind] 路由至 Texture 或 unsupported 占位；[didUpdateWidget] 仅在
/// [VideoSurfaceHandle.playerId] 变化时重建原生纹理。
/// Routes by [VideoSurfaceHandle.kind]; [didUpdateWidget] recreates the native texture only when
/// [VideoSurfaceHandle.playerId] changes.
///
/// # 平台 / Platform
/// - Android：`SurfaceProducer` + `glimagesink`
/// - iOS/macOS/Win/Linux：`appsink` → 像素缓冲 Texture
class TextureVideoSurface extends StatefulWidget {
  /// 绑定 [VideoSurfaceHandle] / Binds [VideoSurfaceHandle].
  const TextureVideoSurface({super.key, required this.handle});

  final VideoSurfaceHandle handle;

  @override
  State<TextureVideoSurface> createState() => _TextureVideoSurfaceState();
}

class _TextureVideoSurfaceState extends State<TextureVideoSurface> {
  int? _textureId;

  @override
  void initState() {
    super.initState();
    if (widget.handle.kind == VideoSurfaceKind.texture) {
      _createTexture();
    }
  }

  @override
  void didUpdateWidget(covariant TextureVideoSurface oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.handle.playerId == widget.handle.playerId) {
      return;
    }
    if (oldWidget.handle.kind == VideoSurfaceKind.texture) {
      _releaseTexture(oldWidget.handle.playerId);
    }
    _textureId = null;
    if (widget.handle.kind == VideoSurfaceKind.texture) {
      _createTexture();
    }
  }

  Future<void> _createTexture() async {
    try {
      final id = await _textureChannel.invokeMethod<int>('createTexture', {
        'playerId': widget.handle.playerId,
      });
      if (mounted && id != null) {
        setState(() => _textureId = id);
      }
    } on PlatformException catch (e) {
      debugPrint('xue_hua_video_player: createTexture failed: $e');
    }
  }

  void _releaseTexture(int playerId) {
    _textureChannel.invokeMethod<void>('disposeTexture', {
      'playerId': playerId,
    });
  }

  @override
  void dispose() {
    if (widget.handle.kind == VideoSurfaceKind.texture) {
      _releaseTexture(widget.handle.playerId);
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (widget.handle.kind == VideoSurfaceKind.unsupported) {
      return ColoredBox(
        color: Colors.black,
        child: Center(
          child: Text('Video not supported on $defaultTargetPlatform'),
        ),
      );
    }

    final id = _textureId;
    if (id == null) {
      return const ColoredBox(color: Colors.black);
    }
    return Texture(textureId: id);
  }
}
