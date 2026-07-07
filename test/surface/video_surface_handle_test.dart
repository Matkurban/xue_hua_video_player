import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/surface/video_surface_handle.dart';

void main() {
  group('VideoSurfaceHandle', () {
    test('kindForPlatform maps mobile and desktop targets', () {
      expect(
        VideoSurfaceHandle.kindForPlatform(TargetPlatform.android),
        VideoSurfaceKind.platformView,
      );
      expect(
        VideoSurfaceHandle.kindForPlatform(TargetPlatform.iOS),
        VideoSurfaceKind.platformView,
      );
      expect(
        VideoSurfaceHandle.kindForPlatform(TargetPlatform.macOS),
        VideoSurfaceKind.platformView,
      );
      expect(
        VideoSurfaceHandle.kindForPlatform(TargetPlatform.windows),
        VideoSurfaceKind.desktopOverlay,
      );
      expect(
        VideoSurfaceHandle.kindForPlatform(TargetPlatform.linux),
        VideoSurfaceKind.desktopOverlay,
      );
      expect(
        VideoSurfaceHandle.kindForPlatform(TargetPlatform.fuchsia),
        VideoSurfaceKind.unsupported,
      );
    });

    test('fromPlayerId preserves playerId', () {
      final handle = VideoSurfaceHandle.fromPlayerId(42);
      expect(handle.playerId, 42);
      expect(handle.kind, VideoSurfaceHandle.kindForPlatform());
    });
  });
}
