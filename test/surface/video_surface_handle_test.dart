import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/surface/video_surface_handle.dart';

void main() {
  group('VideoSurfaceHandle', () {
    test('kindForPlatform maps supported targets to texture', () {
      for (final platform in [
        TargetPlatform.android,
        TargetPlatform.iOS,
        TargetPlatform.macOS,
        TargetPlatform.windows,
        TargetPlatform.linux,
      ]) {
        expect(
          VideoSurfaceHandle.kindForPlatform(platform),
          VideoSurfaceKind.texture,
        );
      }
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
