import 'package:flutter/cupertino.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/controls/fullscreen_config.dart';
import 'package:xue_hua_video_player/src/controls/immersive_controls_state.dart';
import 'package:xue_hua_video_player/src/controls/immersive_hud.dart';
import 'package:xue_hua_video_player/src/controls/video_controls.dart';
import 'package:xue_hua_video_player/src/domain/player_events.dart';

import '../support/fake_playback_controls_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('ImmersiveHud desktop keyboard feedback', () {
    late FakePlaybackControlsModel model;
    late ImmersiveControlsState immersive;

    setUp(() {
      model = FakePlaybackControlsModel(
        initialState: PlayerState.paused,
        initialDuration: const Duration(seconds: 100),
      );
      immersive = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: const VideoControlsFullscreenConfig(),
      );
    });

    tearDown(() {
      immersive.hud.value = null;
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

    testWidgets('seek HUD does not hide center play button on desktop', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        immersive.showHud(
          const ImmersiveHudSnapshot(
            kind: ImmersiveHudKind.seek,
            value: 5,
            forward: true,
          ),
        );
        await tester.pump();

        expect(find.byIcon(Icons.forward_5), findsOneWidget);
        expect(find.byIcon(CupertinoIcons.play_arrow_solid), findsOneWidget);
        await tester.pump(const Duration(seconds: 1));
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('playPause HUD hides center play button while visible', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        immersive.showHud(
          const ImmersiveHudSnapshot(
            kind: ImmersiveHudKind.playPause,
            value: 1.0,
          ),
        );
        await tester.pump();

        expect(find.byIcon(Icons.pause), findsOneWidget);
        expect(find.byIcon(CupertinoIcons.pause_solid), findsNothing);
        expect(find.byIcon(CupertinoIcons.play_arrow_solid), findsNothing);
        await tester.pump(const Duration(seconds: 1));
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('space toggles play pause and shows HUD on desktop', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        await tester.sendKeyEvent(LogicalKeyboardKey.space);
        await tester.pump();
        for (var i = 0; i < 10 && immersive.hud.value == null; i++) {
          await tester.pump(const Duration(milliseconds: 20));
        }

        expect(model.togglePlayPauseCallCount, 1);
        expect(immersive.hud.value?.kind, ImmersiveHudKind.playPause);
        expect(immersive.hud.value?.value, 1.0);
        await tester.pump(const Duration(seconds: 1));
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    test('hudAlignmentFor places seek HUD at top on desktop', () {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        final alignment = hudAlignmentFor(
          const ImmersiveHudSnapshot(
            kind: ImmersiveHudKind.seek,
            value: 5,
            forward: true,
          ),
        );
        expect(alignment, const Alignment(0, -0.35));
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });
  });
}
