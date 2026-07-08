import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/controls/fullscreen_config.dart';
import 'package:xue_hua_video_player/src/controls/immersive_controls_state.dart';
import 'package:xue_hua_video_player/src/controls/video_controls.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';

import '../support/fake_playback_controls_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('VideoControls immersive', () {
    late FakePlaybackControlsModel model;
    late ImmersiveControlsState immersive;

    setUp(() {
      model = FakePlaybackControlsModel(
        initialDuration: const Duration(seconds: 100),
        initialPosition: const Duration(seconds: 30),
      );
      immersive = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: const VideoControlsFullscreenConfig(
          seekStep: Duration(seconds: 5),
          desktopImmersive: true,
          aspectRatioLabels: AspectRatioModeLabels(
            fit: '适应',
            fill: '铺满',
            stretch: '拉伸',
          ),
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

    testWidgets('arrow left seeks backward on desktop immersive', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
        await tester.pump();
        await tester.pump(const Duration(seconds: 1));

        expect(model.seekCallCount, 1);
        expect(model.lastSeek, const Duration(seconds: 25));
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('arrow right seeks forward on desktop immersive', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
        await tester.pump();
        await tester.pump(const Duration(seconds: 1));

        expect(model.seekCallCount, 1);
        expect(model.lastSeek, const Duration(seconds: 35));
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('arrow up increases volume on desktop immersive', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
        await tester.pump();
        await tester.pump(const Duration(seconds: 1));

        expect(model.lastVolume, closeTo(1.0, 0.001));
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('arrow down decreases volume on desktop immersive', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.pump();
        await tester.pump(const Duration(seconds: 1));

        expect(model.lastVolume, closeTo(0.95, 0.001));
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('shows custom aspect ratio label when immersive', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);
        expect(find.text('适应'), findsOneWidget);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('desktopImmersive false ignores arrow keys', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        immersive.dispose();
        immersive = ImmersiveControlsState(
          initialAspectRatioMode: AspectRatioMode.fit,
          fullscreen: const VideoControlsFullscreenConfig(
            desktopImmersive: false,
          ),
        );

        await pumpControls(tester);

        await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
        await tester.pump();

        expect(model.seekCallCount, 0);
        expect(find.text('适应'), findsNothing);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('changing aspectRatioMode updates menu label', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        immersive.aspectRatioMode.value = AspectRatioMode.fill;
        await tester.pump();

        expect(find.text('铺满'), findsOneWidget);
        expect(find.text('适应'), findsNothing);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('mobile tap toggles controls while immersive', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.android;
      try {
        immersive.landscapeLocked.value = true;
        await pumpControls(tester);

        final controlsOpacity = tester
            .widgetList<AnimatedOpacity>(find.byType(AnimatedOpacity))
            .last;
        expect(controlsOpacity.opacity, 1);

        await tester.tapAt(const Offset(160, 120));
        await tester.pump();

        final hidden = tester
            .widgetList<AnimatedOpacity>(find.byType(AnimatedOpacity))
            .last;
        expect(hidden.opacity, 0);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('mobile fullscreen button toggles icon', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.android;
      try {
        await pumpControls(tester);

        expect(find.byIcon(Icons.fullscreen), findsOneWidget);

        await tester.tap(find.byIcon(Icons.fullscreen));
        await tester.pump();

        expect(find.byIcon(Icons.fullscreen_exit), findsOneWidget);
        expect(immersive.landscapeLocked.value, isTrue);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('desktop tap toggles controls while immersive', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        await pumpControls(tester);

        final visible = tester
            .widgetList<AnimatedOpacity>(find.byType(AnimatedOpacity))
            .last;
        expect(visible.opacity, 1);

        await tester.tapAt(const Offset(160, 120));
        await tester.pump();

        final hidden = tester
            .widgetList<AnimatedOpacity>(find.byType(AnimatedOpacity))
            .last;
        expect(hidden.opacity, 0);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });
  });
}
