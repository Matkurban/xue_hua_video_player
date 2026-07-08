import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:signals/signals_flutter.dart';
import 'package:xue_hua_video_player/src/presentation/playback_presentation.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';
import 'package:xue_hua_video_player/src/surface/texture_surface.dart';

import '../support/fake_playback_presentation_model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('Texture surface stability', () {
    const textureChannel = MethodChannel('xue_hua_video_player/texture');

    late FakePlaybackPresentationModel model;
    late FlutterSignal<AspectRatioMode> aspectRatioMode;
    var createTextureCount = 0;
    var disposeTextureCount = 0;

    Future<void> pumpPresentation(WidgetTester tester) async {
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
    }

    setUp(() {
      createTextureCount = 0;
      disposeTextureCount = 0;
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(textureChannel, (call) async {
            switch (call.method) {
              case 'createTexture':
                createTextureCount++;
                return 1;
              case 'disposeTexture':
                disposeTextureCount++;
                return null;
              default:
                return null;
            }
          });
      model = FakePlaybackPresentationModel();
      aspectRatioMode = signal(AspectRatioMode.fit);
    });

    tearDown(() {
      model.dispose();
      aspectRatioMode.dispose();
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(textureChannel, null);
    });

    testWidgets('aspect ratio mode switches do not dispose texture', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      try {
        await pumpPresentation(tester);

        expect(find.byType(TextureVideoSurface), findsOneWidget);
        expect(createTextureCount, 1);
        expect(disposeTextureCount, 0);

        aspectRatioMode.value = AspectRatioMode.fill;
        await tester.pumpAndSettle();
        expect(createTextureCount, 1);
        expect(disposeTextureCount, 0);

        aspectRatioMode.value = AspectRatioMode.stretch;
        await tester.pumpAndSettle();
        expect(createTextureCount, 1);
        expect(disposeTextureCount, 0);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('aspectRatio metadata change does not dispose texture', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      try {
        await pumpPresentation(tester);
        expect(createTextureCount, 1);

        model.setAspectRatio(4 / 3);
        await tester.pumpAndSettle();
        expect(createTextureCount, 1);
        expect(disposeTextureCount, 0);

        aspectRatioMode.value = AspectRatioMode.fill;
        await tester.pumpAndSettle();
        expect(createTextureCount, 1);
        expect(disposeTextureCount, 0);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('buffering state changes do not dispose texture', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      try {
        await pumpPresentation(tester);
        expect(createTextureCount, 1);

        model.setState(PlayerState.buffering);
        model.setBufferingPercent(40);
        await tester.pumpAndSettle();
        expect(find.byType(CircularProgressIndicator), findsOneWidget);
        expect(createTextureCount, 1);
        expect(disposeTextureCount, 0);

        model.setState(PlayerState.playing);
        model.setBufferingPercent(100);
        await tester.pumpAndSettle();
        expect(createTextureCount, 1);
        expect(disposeTextureCount, 0);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('playerId change disposes and recreates texture', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      try {
        await pumpPresentation(tester);
        expect(createTextureCount, 1);

        model.setPlayerId(99);
        await tester.pumpAndSettle();

        expect(disposeTextureCount, 1);
        expect(createTextureCount, 2);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });
  });
}
