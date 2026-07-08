import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/xue_hua_player_controller.dart';
import 'package:xue_hua_video_player/src/xue_hua_video_view.dart';

import '../support/fake_player_command_port.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('XueHuaVideoView', () {
    late FakePlayerCommandPort port;
    late XueHuaPlayerController controller;

    setUp(() {
      port = FakePlayerCommandPort();
      controller = XueHuaPlayerController(port: port);
    });

    tearDown(() async {
      await controller.dispose();
    });

    testWidgets('syncs aspectRatioMode to port on mount', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      await controller.initialize();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: XueHuaVideoView(
                controller: controller,
                aspectRatioMode: AspectRatioMode.fill,
                showControls: false,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(port.lastAspectRatioMode, AspectRatioMode.fill);
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('re-syncs when aspectRatioMode changes', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      await controller.initialize();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: XueHuaVideoView(
                controller: controller,
                aspectRatioMode: AspectRatioMode.fit,
                showControls: false,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(port.lastAspectRatioMode, AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: XueHuaVideoView(
                controller: controller,
                aspectRatioMode: AspectRatioMode.stretch,
                showControls: false,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(port.lastAspectRatioMode, AspectRatioMode.stretch);
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('open resets aspectRatioMode to fit', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      await controller.initialize();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: XueHuaVideoView(
                controller: controller,
                aspectRatioMode: AspectRatioMode.fill,
                showControls: false,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(port.lastAspectRatioMode, AspectRatioMode.fill);

      await controller.open(VideoSource.network('https://example.com/b.mp4'));
      await tester.pumpAndSettle();

      expect(controller.mediaGeneration.value, 1);
      expect(port.lastAspectRatioMode, AspectRatioMode.fit);
      debugDefaultTargetPlatformOverride = null;
    });
  });
}
