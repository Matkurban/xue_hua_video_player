import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/controls/controls_overlay_slots.dart';
import 'package:xue_hua_video_player/src/controls/fullscreen_config.dart';
import 'package:xue_hua_video_player/src/controls/immersive_controls_state.dart';
import 'package:xue_hua_video_player/src/controls/video_controls.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';

import '../support/fake_playback_controls_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('VideoControlsTopBar', () {
    late FakePlaybackControlsModel model;
    late ImmersiveControlsState immersive;

    setUp(() {
      model = FakePlaybackControlsModel();
      immersive = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: const VideoControlsFullscreenConfig(
          desktopImmersive: true,
          aspectRatioLabels: AspectRatioModeLabels(fit: '适应'),
        ),
      );
    });

    tearDown(() {
      model.dispose();
      immersive.dispose();
    });

    Future<void> pumpControls(
      WidgetTester tester, {
      VideoControlsFullscreenConfig fullscreen =
          const VideoControlsFullscreenConfig(
            desktopImmersive: true,
            aspectRatioLabels: AspectRatioModeLabels(fit: '适应'),
          ),
    }) async {
      immersive.dispose();
      immersive = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: fullscreen,
      );

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

    testWidgets('hides aspect ratio menu when showAspectRatioMenu is false', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(
          tester,
          fullscreen: const VideoControlsFullscreenConfig(
            desktopImmersive: true,
            aspectRatioLabels: AspectRatioModeLabels(fit: '适应'),
            overlaySlots: VideoControlsOverlaySlots(showAspectRatioMenu: false),
          ),
        );

        expect(find.text('适应'), findsNothing);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('shows custom actions from overlay slots', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(
          tester,
          fullscreen: const VideoControlsFullscreenConfig(
            desktopImmersive: true,
            overlaySlots: VideoControlsOverlaySlots(
              showAspectRatioMenu: false,
              actions: [Icon(Icons.settings)],
            ),
          ),
        );

        expect(find.byIcon(Icons.settings), findsOneWidget);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('shows custom title and aspect ratio menu when immersive', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(
          tester,
          fullscreen: const VideoControlsFullscreenConfig(
            desktopImmersive: true,
            aspectRatioLabels: AspectRatioModeLabels(fit: '适应'),
            overlaySlots: VideoControlsOverlaySlots(title: Text('Episode 1')),
          ),
        );

        expect(find.text('Episode 1'), findsOneWidget);
        expect(find.text('适应'), findsOneWidget);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });
  });
}
