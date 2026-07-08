import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'video_surface_handle.dart';

/// 原生 Texture 注册 MethodChannel（各平台插件 `xue_hua_video_player/texture`）/ MethodChannel to the native texture registrar.
const MethodChannel _textureChannel = MethodChannel(
  'xue_hua_video_player/texture',
);

/// 通过 Flutter 外部 [Texture] 渲染 player 视频 / Renders a player's video through a Flutter external [Texture].
///
/// 创建时调用 `createTexture` 注册 texture id；dispose 时调用 `disposeTexture` 释放。
/// On create invokes `createTexture` for a texture id; on dispose invokes `disposeTexture`.
///
/// # 平台 / Platform
/// - Android：`SurfaceProducer` + `glimagesink`
/// - iOS/macOS/Win/Linux：`appsink` → 像素缓冲 Texture
class TextureVideoSurface extends StatefulWidget {
  /// 绑定 [VideoSurfaceHandle.playerId] / Binds [VideoSurfaceHandle.playerId].
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
    _createTexture();
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

  @override
  void dispose() {
    _textureChannel.invokeMethod<void>('disposeTexture', {
      'playerId': widget.handle.playerId,
    });
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final id = _textureId;
    if (id == null) {
      return const ColoredBox(color: Colors.black);
    }
    return Texture(textureId: id);
  }
}
