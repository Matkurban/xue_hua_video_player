import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/controls/controls_overlay_slots.dart';
import 'package:xue_hua_video_player/src/controls/fullscreen_config.dart';
import 'package:xue_hua_video_player/src/controls/immersive_controls_state.dart';
import 'package:xue_hua_video_player/src/controls/video_controls.dart';
import 'package:xue_hua_video_player/src/domain/player_events.dart';

import '../support/fake_playback_controls_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('VideoControls auto-hide', () {
    late FakePlaybackControlsModel model;
    late ImmersiveControlsState immersive;

    setUp(() {
      model = FakePlaybackControlsModel(initialState: PlayerState.playing);
      immersive = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: const VideoControlsFullscreenConfig(
          desktopImmersive: true,
          aspectRatioLabels: AspectRatioModeLabels(fit: '适应'),
          overlaySlots: VideoControlsOverlaySlots(
            title: Text('Episode 1'),
            showAspectRatioMenu: true,
          ),
        ),
      );
    });

    tearDown(() {
      model.dispose();
      immersive.dispose();
    });

    Future<void> pumpControls(
      WidgetTester tester, {
      Duration autoHide = const Duration(seconds: 3),
    }) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 800,
              height: 450,
              child: Stack(
                children: [
                  VideoControls(
                    model: model,
                    immersive: immersive,
                    autoHide: autoHide,
                  ),
                ],
              ),
            ),
          ),
        ),
      );
      await tester.pump();
    }

    AnimatedOpacity opacityChrome(WidgetTester tester) {
      return tester.widget<AnimatedOpacity>(
        find.byKey(const ValueKey('video-controls-opacity')),
      );
    }

    testWidgets(
      'hides top bar with bottom chrome after autoHide while playing',
      (tester) async {
        debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
        try {
          await pumpControls(
            tester,
            autoHide: const Duration(milliseconds: 50),
          );

          expect(find.text('Episode 1'), findsOneWidget);
          expect(opacityChrome(tester).opacity, 1.0);

          await tester.pump(const Duration(milliseconds: 50));
          await tester.pump(); // signal rebuild
          await tester.pump(const Duration(milliseconds: 200)); // fade

          expect(opacityChrome(tester).opacity, 0.0);
          final ignore = tester.widget<IgnorePointer>(
            find.descendant(
              of: find.byKey(const ValueKey('video-controls-opacity')),
              matching: find.byType(IgnorePointer),
            ),
          );
          expect(ignore.ignoring, isTrue);
        } finally {
          debugDefaultTargetPlatformOverride = null;
        }
      },
    );

    testWidgets('desktop mouse hover shows chrome again after auto-hide', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester, autoHide: const Duration(milliseconds: 50));

        await tester.pump(const Duration(milliseconds: 50));
        await tester.pump();
        expect(opacityChrome(tester).opacity, 0.0);

        final gesture = await tester.createGesture(
          kind: PointerDeviceKind.mouse,
        );
        await gesture.addPointer(location: const Offset(100, 100));
        addTearDown(gesture.removePointer);
        await tester.pump();
        await gesture.moveBy(const Offset(20, 10));
        await tester.pump();

        expect(opacityChrome(tester).opacity, 1.0);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });
  });
}
