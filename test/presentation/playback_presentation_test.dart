import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/presentation/playback_presentation.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';

import '../support/fake_playback_presentation_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('PlaybackPresentation', () {
    late FakePlaybackPresentationModel model;

    tearDown(() {
      model.dispose();
    });

    testWidgets('syncs aspectRatioMode on mount', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.fuchsia;
      model = FakePlaybackPresentationModel();

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: AspectRatioMode.fill,
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

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: AspectRatioMode.fit,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(model.lastAspectRatioMode, AspectRatioMode.fit);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: AspectRatioMode.stretch,
              ),
            ),
          ),
        ),
      );
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

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: AspectRatioMode.fit,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(CircularProgressIndicator), findsOneWidget);

      model.setState(PlayerState.playing);
      await tester.pumpAndSettle();

      expect(find.byType(CircularProgressIndicator), findsNothing);
      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('hides surface when playerId is null', (tester) async {
      model = FakePlaybackPresentationModel(playerId: null);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 320,
              height: 180,
              child: PlaybackPresentation(
                model: model,
                aspectRatioMode: AspectRatioMode.fit,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(AspectRatio), findsNothing);
    });
  });
}
