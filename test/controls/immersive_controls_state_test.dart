import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/controls/fullscreen_config.dart';
import 'package:xue_hua_video_player/src/controls/immersive_controls_state.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';

void main() {
  group('ImmersiveControlsState', () {
    late ImmersiveControlsState state;

    setUp(() {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    });

    tearDown(() {
      debugDefaultTargetPlatformOverride = null;
      state.dispose();
    });

    test('immersiveActive follows desktopImmersive on desktop hosts', () {
      state = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: const VideoControlsFullscreenConfig(desktopImmersive: true),
      );
      expect(state.immersiveActive.value, isTrue);

      state.dispose();
      state = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: const VideoControlsFullscreenConfig(desktopImmersive: false),
      );
      expect(state.immersiveActive.value, isFalse);
    });

    test('fullscreen signal updates immersiveActive on desktop', () {
      state = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: const VideoControlsFullscreenConfig(desktopImmersive: true),
      );
      state.fullscreen.value = const VideoControlsFullscreenConfig(
        desktopImmersive: false,
      );
      expect(state.immersiveActive.value, isFalse);
    });

    test('landscapeLocked toggles aspectRatioMode independently', () {
      state = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: const VideoControlsFullscreenConfig(),
      );

      state.landscapeLocked.value = true;
      state.aspectRatioMode.value = AspectRatioMode.fill;

      expect(state.landscapeLocked.value, isTrue);
      expect(state.aspectRatioMode.value, AspectRatioMode.fill);
    });

    test('hud signal starts null', () {
      state = ImmersiveControlsState(
        initialAspectRatioMode: AspectRatioMode.fit,
        fullscreen: const VideoControlsFullscreenConfig(),
      );
      expect(state.hud.value, isNull);
    });
  });

  group('AspectRatioModeLabels', () {
    test('label returns custom strings', () {
      const labels = AspectRatioModeLabels(
        fit: 'Fit',
        fill: 'Fill',
        stretch: 'Stretch',
      );
      expect(labels.label(AspectRatioMode.fit), 'Fit');
      expect(labels.label(AspectRatioMode.fill), 'Fill');
      expect(labels.label(AspectRatioMode.stretch), 'Stretch');
    });
  });
}
