import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Platform view type registered by native plugin code.
const String kXueHuaVideoViewType = 'xue_hua_video_player/view';

const MethodChannel _desktopOverlayChannel =
    MethodChannel('xue_hua_video_player/desktop_overlay');

/// Builds the platform-appropriate video surface for [playerId].
Widget buildXueHuaVideoPlatformView({required int playerId}) {
  final creationParams = <String, dynamic>{'playerId': playerId};
  const paramsCodec = StandardMessageCodec();

  if (defaultTargetPlatform == TargetPlatform.android) {
    return AndroidView(
      viewType: kXueHuaVideoViewType,
      creationParams: creationParams,
      creationParamsCodec: paramsCodec,
      gestureRecognizers: const <Factory<OneSequenceGestureRecognizer>>{},
    );
  }
  if (defaultTargetPlatform == TargetPlatform.iOS) {
    return UiKitView(
      viewType: kXueHuaVideoViewType,
      creationParams: creationParams,
      creationParamsCodec: paramsCodec,
      gestureRecognizers: const <Factory<OneSequenceGestureRecognizer>>{},
    );
  }
  if (defaultTargetPlatform == TargetPlatform.macOS) {
    return AppKitView(
      viewType: kXueHuaVideoViewType,
      creationParams: creationParams,
      creationParamsCodec: paramsCodec,
      gestureRecognizers: const <Factory<OneSequenceGestureRecognizer>>{},
    );
  }
  if (defaultTargetPlatform == TargetPlatform.windows ||
      defaultTargetPlatform == TargetPlatform.linux) {
    return _DesktopVideoOverlay(playerId: playerId);
  }
  return ColoredBox(
    color: Colors.black,
    child: Center(
      child: Text('Video not supported on $defaultTargetPlatform'),
    ),
  );
}

/// Positions a native overlay window for GStreamer on desktop platforms where
/// Flutter PlatformView embedding is not yet available in the framework.
class _DesktopVideoOverlay extends StatefulWidget {
  const _DesktopVideoOverlay({required this.playerId});

  final int playerId;

  @override
  State<_DesktopVideoOverlay> createState() => _DesktopVideoOverlayState();
}

class _DesktopVideoOverlayState extends State<_DesktopVideoOverlay> {
  @override
  void initState() {
    super.initState();
    _desktopOverlayChannel.invokeMethod<void>(
      'attach',
      <String, dynamic>{'playerId': widget.playerId},
    );
    WidgetsBinding.instance.addPostFrameCallback((_) => _syncBounds());
  }

  @override
  void dispose() {
    _desktopOverlayChannel.invokeMethod<void>(
      'detach',
      <String, dynamic>{'playerId': widget.playerId},
    );
    super.dispose();
  }

  void _syncBounds() {
    if (!mounted) return;
    final box = context.findRenderObject();
    if (box is! RenderBox || !box.hasSize) return;
    final offset = box.localToGlobal(Offset.zero);
    final size = box.size;
    _desktopOverlayChannel.invokeMethod<void>(
      'setBounds',
      <String, dynamic>{
        'playerId': widget.playerId,
        'x': offset.dx,
        'y': offset.dy,
        'width': size.width,
        'height': size.height,
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    WidgetsBinding.instance.addPostFrameCallback((_) => _syncBounds());
    return const SizedBox.expand();
  }
}
