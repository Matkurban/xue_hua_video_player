import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/surface/desktop_overlay.dart';
import 'package:xue_hua_video_player/src/surface/desktop_overlay_bounds.dart';
import 'package:xue_hua_video_player/src/surface/video_surface_handle.dart';

void main() {
  group('DesktopVideoOverlay', () {
    testWidgets('calls attach, setBounds, and detach on fake client', (
      tester,
    ) async {
      final fake = _FakeDesktopOverlayClient();
      const handle = VideoSurfaceHandle(
        playerId: 7,
        kind: VideoSurfaceKind.desktopOverlay,
      );

      await tester.pumpWidget(
        MaterialApp(
          home: Center(
            child: SizedBox(
              width: 200,
              height: 100,
              child: DesktopVideoOverlay(handle: handle, client: fake),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(fake.attachCalls, [7]);
      expect(fake.setBoundsCalls, isNotEmpty);
      expect(fake.setBoundsCalls.first.playerId, 7);
      expect(fake.setBoundsCalls.first.bounds.width, 200);
      expect(fake.setBoundsCalls.first.bounds.height, 100);

      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pumpAndSettle();

      expect(fake.detachCalls, [7]);
    });
  });
}

class _FakeDesktopOverlayClient implements DesktopOverlayClient {
  final attachCalls = <int>[];
  final detachCalls = <int>[];
  final setBoundsCalls = <({int playerId, DesktopOverlayBounds bounds})>[];

  @override
  Future<void> attach(int playerId) async {
    attachCalls.add(playerId);
  }

  @override
  Future<void> detach(int playerId) async {
    detachCalls.add(playerId);
  }

  @override
  Future<void> setBounds(int playerId, DesktopOverlayBounds bounds) async {
    setBoundsCalls.add((playerId: playerId, bounds: bounds));
  }
}
