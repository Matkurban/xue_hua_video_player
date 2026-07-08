import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'video_surface_handle.dart';

/// MethodChannel to the native texture registrar (see each platform plugin's
/// `xue_hua_video_player/texture` channel).
const MethodChannel _textureChannel = MethodChannel(
  'xue_hua_video_player/texture',
);

/// Renders a player's video through a Flutter external [Texture].
///
/// On create it asks the native plugin to register a texture for the player id
/// (returning a texture id) and shows a [Texture]; on dispose it releases it.
class TextureVideoSurface extends StatefulWidget {
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
