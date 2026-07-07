import 'package:flutter/foundation.dart';
import 'package:flutter/rendering.dart';

/// Screen-space bounds for positioning a native desktop overlay window.
@immutable
class DesktopOverlayBounds {
  const DesktopOverlayBounds({
    required this.x,
    required this.y,
    required this.width,
    required this.height,
  });

  final double x;
  final double y;
  final double width;
  final double height;

  Map<String, dynamic> toChannelArgs(int playerId) {
    return <String, dynamic>{
      'playerId': playerId,
      'x': x,
      'y': y,
      'width': width,
      'height': height,
    };
  }

  /// Computes global bounds from a laid-out [RenderBox], or `null` when invalid.
  static DesktopOverlayBounds? fromRenderBox(RenderBox box) {
    if (!box.hasSize) return null;
    final offset = box.localToGlobal(Offset.zero);
    final size = box.size;
    if (size.width <= 0 || size.height <= 0) return null;
    return DesktopOverlayBounds(
      x: offset.dx,
      y: offset.dy,
      width: size.width,
      height: size.height,
    );
  }
}
