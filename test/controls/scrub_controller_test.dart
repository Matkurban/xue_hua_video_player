import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/controls/scrub_controller.dart';

import '../support/fake_playback_controls_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('ScrubController', () {
    late FakePlaybackControlsModel model;
    late ScrubController scrub;
    var interactCount = 0;

    setUp(() {
      interactCount = 0;
      model = FakePlaybackControlsModel(
        initialDuration: const Duration(seconds: 100),
        initialPosition: const Duration(seconds: 10),
      );
      scrub = ScrubController(model: model, onInteract: () => interactCount++);
    });

    tearDown(() {
      scrub.dispose();
      model.dispose();
    });

    test('sliderValue follows position when not scrubbing', () {
      expect(scrub.sliderValue(100_000, 10_000), closeTo(0.1, 0.001));
    });

    test('onSeekChanged pins slider value while dragging', () {
      scrub.onSeekStart();
      scrub.onSeekChanged(0.5, 100_000);

      expect(scrub.isScrubbing, isTrue);
      expect(scrub.sliderValue(100_000, 10_000), 0.5);
      expect(interactCount, greaterThan(0));
    });

    test('onSeekEnd issues seek when position is far from target', () {
      scrub.onSeekEnd(0.5, 100_000);

      expect(model.seekCallCount, 1);
      expect(model.lastSeek, const Duration(seconds: 50));
      expect(scrub.isScrubbing, isTrue);
    });

    test('onSeekEnd clears immediately when already near target', () {
      model.setPosition(const Duration(seconds: 50));

      scrub.onSeekEnd(0.5, 100_000);

      expect(model.seekCallCount, 0);
      expect(scrub.isScrubbing, isFalse);
    });

    test('position catch-up clears pinned value after seek', () {
      scrub.onSeekEnd(0.5, 100_000);
      expect(scrub.isScrubbing, isTrue);

      model.setPosition(const Duration(seconds: 50));

      expect(scrub.isScrubbing, isFalse);
    });
  });
}
