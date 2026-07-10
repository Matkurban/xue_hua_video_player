import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'video_surface_handle.dart';

/// 原生 Texture 注册 MethodChannel（各平台插件 `xue_hua_video_player/texture`）/ MethodChannel to the native texture registrar.
const MethodChannel _textureChannel = MethodChannel(
  'xue_hua_video_player/texture',
);

/// Releases the native Flutter texture for [playerId].
///
/// Texture lifetime follows the **player**, not the view: call from
/// [PlaybackSession.dispose] (or when swapping player ids). Do **not** call
/// from [TextureVideoSurface] dispose — Hero / SignalBuilder remounts would
/// tear down a still-playing SurfaceProducer (abandoned BufferQueue).
Future<void> disposeNativePlayerTexture(int playerId) async {
  try {
    await _textureChannel.invokeMethod<void>('disposeTexture', {
      'playerId': playerId,
    });
  } on MissingPluginException {
    // Unit tests / hosts without the platform plugin.
  } on PlatformException catch (e) {
    debugPrint('xue_hua_video_player: disposeTexture failed: $e');
  }
}

/// 通过 Flutter 外部 [Texture] 渲染 player 视频 / Renders a player's video through a Flutter external [Texture].
///
/// 按 [VideoSurfaceHandle.kind] 路由至 Texture 或 unsupported 占位；[didUpdateWidget] 仅在
/// [VideoSurfaceHandle.playerId] 变化时重建原生纹理。
/// Routes by [VideoSurfaceHandle.kind]; [didUpdateWidget] recreates the native texture only when
/// [VideoSurfaceHandle.playerId] changes.
///
/// # 平台 / Platform
/// - Android：`SurfaceProducer` + `glimagesink`；拟合视频矩形的物理像素驱动
///   `setSize`（与 Texture / 视频同宽高比；勿用整屏竖屏视口，否则高度被压）
/// - iOS/macOS/Win/Linux：`appsink` → 像素缓冲 Texture
class TextureVideoSurface extends StatefulWidget {
  /// 绑定 [VideoSurfaceHandle] / Binds [VideoSurfaceHandle].
  ///
  /// [androidLayoutSize] 为 Android buffer 逻辑尺寸（拟合后的视频矩形，与
  /// Texture 同宽高比）。换算物理像素后调用 `SurfaceProducer.setSize`。
  /// 勿传 FittedBox 单位盒（h=1），也勿传整屏竖屏视口。
  /// 为 null 或不可用（≤1）时回退到 [MediaQuery] 拟合尺寸。
  const TextureVideoSurface({
    super.key,
    required this.handle,
    this.androidLayoutSize,
  });

  final VideoSurfaceHandle handle;

  /// Android buffer 逻辑尺寸（视频宽高比）；为 null / 不可用时回退 MediaQuery。
  final Size? androidLayoutSize;

  @override
  State<TextureVideoSurface> createState() => _TextureVideoSurfaceState();
}

class _TextureVideoSurfaceState extends State<TextureVideoSurface> {
  int? _textureId;
  int? _syncedPhysicalWidth;
  int? _syncedPhysicalHeight;

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
    if (oldWidget.handle.playerId != widget.handle.playerId) {
      if (oldWidget.handle.kind == VideoSurfaceKind.texture) {
        unawaited(disposeNativePlayerTexture(oldWidget.handle.playerId));
      }
      _textureId = null;
      _syncedPhysicalWidth = null;
      _syncedPhysicalHeight = null;
      if (widget.handle.kind == VideoSurfaceKind.texture) {
        _createTexture();
      }
      return;
    }
    if (oldWidget.androidLayoutSize != widget.androidLayoutSize &&
        defaultTargetPlatform == TargetPlatform.android &&
        _textureId != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          _syncAndroidBufferSize();
        }
      });
    }
  }

  Future<void> _createTexture() async {
    try {
      final id = await _textureChannel.invokeMethod<int>('createTexture', {
        'playerId': widget.handle.playerId,
      });
      if (mounted && id != null) {
        setState(() => _textureId = id);
        if (defaultTargetPlatform == TargetPlatform.android) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) {
              _syncAndroidBufferSize();
            }
          });
        }
      }
    } on PlatformException catch (e) {
      debugPrint('xue_hua_video_player: createTexture failed: $e');
    }
  }

  /// Prefer [androidLayoutSize] when usable; else MediaQuery-fitted 16:9.
  void _syncAndroidBufferSize() {
    final layout = widget.androidLayoutSize;
    if (layout != null && layout.width > 1 && layout.height > 1) {
      _syncAndroidTextureSize(layout.width, layout.height);
      return;
    }
    final screen = MediaQuery.sizeOf(context);
    if (screen.width <= 1 || screen.height <= 1) {
      return;
    }
    // Contain-fit 16:9 into the screen (Hero / zero layout fallback).
    const ratio = 16 / 9;
    final fitted = applyBoxFit(
      BoxFit.contain,
      const Size(ratio, 1),
      screen,
    ).destination;
    _syncAndroidTextureSize(fitted.width, fitted.height);
  }

  /// Android: SurfaceProducer defaults to 1×1 until the plugin calls setSize.
  /// Drive size from Flutter layout (physical pixels), never from video caps.
  void _syncAndroidTextureSize(double logicalW, double logicalH) {
    if (logicalW <= 1 || logicalH <= 1) {
      return;
    }
    final dpr = MediaQuery.devicePixelRatioOf(context);
    final physicalW = (logicalW * dpr).round();
    final physicalH = (logicalH * dpr).round();
    if (physicalW <= 1 || physicalH <= 1) {
      return;
    }
    if (physicalW == _syncedPhysicalWidth &&
        physicalH == _syncedPhysicalHeight) {
      return;
    }
    _syncedPhysicalWidth = physicalW;
    _syncedPhysicalHeight = physicalH;
    _textureChannel.invokeMethod<void>('syncTextureSize', {
      'playerId': widget.handle.playerId,
      'width': physicalW,
      'height': physicalH,
    });
  }

  void _syncAndroidFromConstraints(BoxConstraints constraints) {
    if (!constraints.hasBoundedWidth || !constraints.hasBoundedHeight) {
      return;
    }
    if (constraints.maxWidth <= 1 || constraints.maxHeight <= 1) {
      _syncAndroidBufferSize();
      return;
    }
    _syncAndroidTextureSize(constraints.maxWidth, constraints.maxHeight);
  }

  @override
  void dispose() {
    // Texture is owned by the player session — do not disposeTexture here.
    // View remounts (Hero / SignalBuilder) must keep the SurfaceProducer alive.
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

    final texture = Texture(textureId: id);
    if (defaultTargetPlatform != TargetPlatform.android) {
      return texture;
    }

    // Prefer outer layout size; null / unusable → MediaQuery fallback.
    // When androidLayoutSize is omitted, fall back to own LayoutBuilder.
    if (widget.androidLayoutSize != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          _syncAndroidBufferSize();
        }
      });
      return texture;
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) {
            _syncAndroidFromConstraints(constraints);
          }
        });
        return texture;
      },
    );
  }
}
