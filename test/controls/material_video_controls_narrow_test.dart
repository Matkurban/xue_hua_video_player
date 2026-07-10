import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:signals/signals_flutter.dart';
import 'package:xue_hua_video_player/src/controls/material_video_controls.dart';
import 'package:xue_hua_video_player/src/theme/video_controls_theme.dart';

import '../support/fake_playback_controls_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('Material bottom bar does not overflow at Hero-narrow width', (
    tester,
  ) async {
    final model = FakePlaybackControlsModel();
    final landscapeLocked = signal(false);
    addTearDown(() {
      model.dispose();
      landscapeLocked.dispose();
    });

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 120,
            height: 200,
            child: MaterialVideoControls(
              model: model,
              theme: VideoControlsTheme.material(),
              onInteract: () {},
              showFullscreenButton: true,
              landscapeLocked: landscapeLocked,
              onFullscreenToggle: () {},
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    expect(tester.takeException(), isNull);
    expect(find.textContaining('/'), findsOneWidget);
    expect(find.byIcon(Icons.loop), findsNothing);
    expect(find.text('1.0x'), findsNothing);
    expect(find.byIcon(Icons.volume_up), findsOneWidget);
    expect(find.byIcon(Icons.fullscreen), findsOneWidget);
  });
}
