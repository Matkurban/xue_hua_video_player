import 'package:flutter/cupertino.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:signals/signals_flutter.dart';
import 'package:xue_hua_video_player/src/controls/buffering_indicator.dart';
import 'package:xue_hua_video_player/src/enum/video_controls_style.dart';
import 'package:xue_hua_video_player/src/presentation/playback_presentation.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';
import 'package:xue_hua_video_player/src/theme/video_controls_theme.dart';

import '../support/fake_playback_presentation_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('PlaybackPresentation', () {
    late FakePlaybackPresentationModel model;
    late FlutterSignal<AspectRatioMode> aspectRatioMode;

    tearDown(() {
      model.dispose();
      aspectRatioMode.dispose();
    });

    testWidgets('syncs aspectRatioMode on mount', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      model = FakePlaybackPresentationModel();
      aspectRatioMode = signal(AspectRatioMode.fill);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: aspectRatioMode,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(model.lastAspectRatioMode, AspectRatioMode.fill);
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('re-syncs when aspectRatioMode changes', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      model = FakePlaybackPresentationModel();
      aspectRatioMode = signal(AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: aspectRatioMode,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(model.lastAspectRatioMode, AspectRatioMode.fit);

      aspectRatioMode.value = AspectRatioMode.stretch;
      await tester.pumpAndSettle();

      expect(model.lastAspectRatioMode, AspectRatioMode.stretch);
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('shows loading chrome while buffering', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      model = FakePlaybackPresentationModel(
        state: PlayerState.buffering,
        bufferingPercent: 50,
      );
      aspectRatioMode = signal(AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: aspectRatioMode,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(CircularProgressIndicator), findsOneWidget);
      expect(find.text('50%'), findsOneWidget);

      final indicatorBox = tester.widget<SizedBox>(
        find.descendant(
          of: find.byType(BufferingIndicator),
          matching: find.byWidgetPredicate(
            (widget) =>
                widget is SizedBox &&
                widget.width == BufferingIndicator.materialIndicatorSize &&
                widget.height == BufferingIndicator.materialIndicatorSize,
          ),
        ),
      );
      expect(indicatorBox.width, 36);
      expect(indicatorBox.height, 36);

      model.setState(PlayerState.playing);
      model.setBufferingPercent(100);
      await tester.pumpAndSettle();

      expect(find.byType(CircularProgressIndicator), findsNothing);
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('shows loading chrome during rebuffer while playing', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      model = FakePlaybackPresentationModel(
        state: PlayerState.playing,
        bufferingPercent: 50,
      );
      aspectRatioMode = signal(AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: aspectRatioMode,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(CircularProgressIndicator), findsOneWidget);
      expect(find.text('50%'), findsOneWidget);
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('uses Cupertino buffering indicator when requested', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      model = FakePlaybackPresentationModel(
        state: PlayerState.buffering,
        bufferingPercent: 42,
      );
      aspectRatioMode = signal(AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: aspectRatioMode,
                controlsStyle: VideoControlsStyle.cupertino,
              ),
            ),
          ),
        ),
      );
      await tester.pump();

      expect(find.byType(CupertinoActivityIndicator), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsNothing);
      expect(find.text('42%'), findsOneWidget);
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('default theme uses no fullscreen buffering scrim', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      model = FakePlaybackPresentationModel(
        state: PlayerState.buffering,
        bufferingPercent: 50,
      );
      aspectRatioMode = signal(AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: aspectRatioMode,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(
        find.byWidgetPredicate(
          (widget) =>
              widget is ColoredBox && widget.color == const Color(0x61000000),
        ),
        findsNothing,
      );
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('shows buffering scrim when theme configures one', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      model = FakePlaybackPresentationModel(
        state: PlayerState.buffering,
        bufferingPercent: 50,
      );
      aspectRatioMode = signal(AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          theme: ThemeData(
            extensions: [
              VideoControlsTheme.material().copyWith(
                bufferingScrimColor: Colors.black38,
              ),
            ],
          ),
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: aspectRatioMode,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(
        find.byWidgetPredicate(
          (widget) => widget is ColoredBox && widget.color == Colors.black38,
        ),
        findsOneWidget,
      );
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('fit mode letterboxes with AspectRatio', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      model = FakePlaybackPresentationModel();
      aspectRatioMode = signal(AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 400,
              height: 300,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: aspectRatioMode,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(FittedBox), findsOneWidget);
      expect(
        tester.widget<FittedBox>(find.byType(FittedBox)).fit,
        BoxFit.contain,
      );
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('aspect ratio modes map to distinct BoxFit values', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      model = FakePlaybackPresentationModel();
      aspectRatioMode = signal(AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 400,
              height: 300,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: aspectRatioMode,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(
        tester.widget<FittedBox>(find.byType(FittedBox)).fit,
        BoxFit.contain,
      );

      aspectRatioMode.value = AspectRatioMode.fill;
      await tester.pumpAndSettle();
      expect(
        tester.widget<FittedBox>(find.byType(FittedBox)).fit,
        BoxFit.cover,
      );

      aspectRatioMode.value = AspectRatioMode.stretch;
      await tester.pumpAndSettle();
      expect(tester.widget<FittedBox>(find.byType(FittedBox)).fit, BoxFit.fill);

      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('hides surface when playerId is null', (tester) async {
      model = FakePlaybackPresentationModel(playerId: null);
      aspectRatioMode = signal(AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: aspectRatioMode,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(FittedBox), findsNothing);
    });
  });
}
