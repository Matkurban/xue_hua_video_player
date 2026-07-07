import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/controls/playback_progress_slider.dart';
import 'package:xue_hua_video_player/src/controls/scrub_controller.dart';

import '../support/fake_playback_controls_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('PlaybackProgressSlider', () {
    late FakePlaybackControlsModel model;
    late ScrubController scrub;

    setUp(() {
      model = FakePlaybackControlsModel(
        initialDuration: const Duration(seconds: 60),
        initialPosition: const Duration(seconds: 15),
      );
      scrub = ScrubController(model: model, onInteract: () {});
    });

    tearDown(() {
      scrub.dispose();
      model.dispose();
    });

    testWidgets('builds slider with enabled callbacks when seekable', (
      tester,
    ) async {
      PlaybackSliderSnapshot? lastSnapshot;

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: PlaybackProgressSlider(
              model: model,
              scrub: scrub,
              builder: (context, snapshot) {
                lastSnapshot = snapshot;
                return Slider(
                  value: snapshot.displayValue,
                  onChangeStart: snapshot.enabled
                      ? (_) => snapshot.onSeekStart?.call()
                      : null,
                  onChanged: snapshot.onSeekChanged,
                  onChangeEnd: snapshot.onSeekEnd,
                );
              },
            ),
          ),
        ),
      );
      await tester.pump();

      expect(lastSnapshot, isNotNull);
      expect(lastSnapshot!.enabled, isTrue);
      expect(lastSnapshot!.displayValue, closeTo(0.25, 0.01));
      expect(find.byType(Slider), findsOneWidget);
    });

    testWidgets('disables callbacks when not seekable', (tester) async {
      final nonSeekable = FakePlaybackControlsModel(initialSeekable: false);
      final localScrub = ScrubController(model: nonSeekable, onInteract: () {});
      addTearDown(() {
        localScrub.dispose();
        nonSeekable.dispose();
      });

      PlaybackSliderSnapshot? lastSnapshot;

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: PlaybackProgressSlider(
              model: nonSeekable,
              scrub: localScrub,
              builder: (context, snapshot) {
                lastSnapshot = snapshot;
                return Slider(
                  value: snapshot.displayValue,
                  onChanged: snapshot.onSeekChanged,
                );
              },
            ),
          ),
        ),
      );
      await tester.pump();

      expect(lastSnapshot!.enabled, isFalse);
      expect(lastSnapshot!.onSeekStart, isNull);
      expect(lastSnapshot!.onSeekChanged, isNull);
      expect(lastSnapshot!.onSeekEnd, isNull);
    });
  });
}
