import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/controls/fullscreen_config.dart';
import 'package:xue_hua_video_player/src/controls/immersive_controls_state.dart';
import 'package:xue_hua_video_player/src/controls/video_controls.dart';
import 'package:xue_hua_video_player/src/enum/video_rotation.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';

import '../support/fake_playback_controls_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('VideoOrientationMenuButton', () {
    late FakePlaybackControlsModel model;
    late ImmersiveControlsState immersive;

    setUp(() {
      model = FakePlaybackControlsModel(supportsOrientation: true);
      immersive = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: const VideoControlsFullscreenConfig(
          showOrientationMenu: true,
        ),
      );
    });

    tearDown(() {
      model.dispose();
      immersive.dispose();
    });

    Future<void> pumpControls(WidgetTester tester) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: Stack(
              children: [VideoControls(model: model, immersive: immersive)],
            ),
          ),
        ),
      );
      await tester.pump();
    }

    testWidgets('shows orientation button on mobile fullscreen by default', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.android;
      try {
        immersive.landscapeLocked.value = true;
        await pumpControls(tester);

        expect(find.byIcon(Icons.screen_rotation), findsOneWidget);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('hides orientation button when showOrientationMenu is false', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.android;
      try {
        immersive.dispose();
        immersive = ImmersiveControlsState(
          initialAspectRatioMode: AspectRatioMode.fit,
          fullscreen: const VideoControlsFullscreenConfig(
            showOrientationMenu: false,
          ),
        );
        immersive.landscapeLocked.value = true;
        await pumpControls(tester);

        expect(find.byIcon(Icons.screen_rotation), findsNothing);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('hides orientation button when not in fullscreen', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.android;
      try {
        await pumpControls(tester);

        expect(find.byIcon(Icons.screen_rotation), findsNothing);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('shows orientation button on desktop when immersive', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        expect(find.byIcon(Icons.screen_rotation), findsOneWidget);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('rotation panel uses one SegmentedButton', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        await tester.tap(find.byIcon(Icons.screen_rotation));
        await tester.pumpAndSettle();

        expect(find.byType(SegmentedButton<VideoRotation>), findsOneWidget);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('rotation panel updates rotation', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        await tester.tap(find.byIcon(Icons.screen_rotation));
        await tester.pumpAndSettle();

        await tester.tap(find.text('90°'));
        await tester.pumpAndSettle();

        expect(model.lastVideoRotation, VideoRotation.deg90);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });
  });
}
