import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/player/state_store.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('PlayerStateStore', () {
    late PlayerStateStore store;

    setUp(() {
      store = PlayerStateStore();
    });

    tearDown(() {
      store.dispose();
    });

    PlayerEvent event({
      required PlayerEventKind kind,
      PlayerState state = PlayerState.idle,
      int positionMs = 0,
      int durationMs = 0,
      int width = 0,
      int height = 0,
      int bufferingPercent = 0,
      String message = '',
      bool isSeekable = true,
    }) {
      return PlayerEvent(
        kind: kind,
        positionMs: positionMs,
        durationMs: durationMs,
        width: width,
        height: height,
        bufferingPercent: bufferingPercent,
        state: state,
        message: message,
        fps: 0,
        pixelAspectWidth: 1,
        pixelAspectHeight: 1,
        displayAspectWidth: 16,
        displayAspectHeight: 9,
        interlaced: false,
        colorMatrix: '',
        colorRange: '',
        hdrFormat: '',
        isSeekable: isSeekable,
      );
    }

    test('stateChanged updates state and clears buffering on playing', () {
      store.apply(event(kind: PlayerEventKind.buffering, bufferingPercent: 42));
      expect(store.bufferingPercent.value, 42);

      store.apply(
        event(kind: PlayerEventKind.stateChanged, state: PlayerState.playing),
      );
      expect(store.state.value, PlayerState.playing);
      expect(store.bufferingPercent.value, 100);
    });

    test('eos marks completed and snaps position to duration', () {
      store.apply(
        event(kind: PlayerEventKind.durationChanged, durationMs: 5000),
      );
      store.apply(
        event(kind: PlayerEventKind.positionChanged, positionMs: 4800),
      );
      store.apply(event(kind: PlayerEventKind.eos));

      expect(store.state.value, PlayerState.completed);
      expect(store.position.value, const Duration(milliseconds: 5000));
      expect(store.isCompleted.value, isTrue);
    });

    test('metadataChanged drives aspectRatio computed signal', () {
      store.apply(
        event(kind: PlayerEventKind.metadataChanged, width: 1920, height: 1080),
      );
      expect(store.aspectRatio.value, closeTo(16 / 9, 0.001));
    });

    test('videoSize fallback when metadata lacks display aspect', () {
      store.apply(
        event(kind: PlayerEventKind.videoSize, width: 640, height: 480),
      );
      expect(store.aspectRatio.value, closeTo(4 / 3, 0.001));
    });

    test('applyError sets error state', () {
      store.applyError('boom');
      expect(store.error.value, 'boom');
      expect(store.state.value, PlayerState.error);
    });

    test('resetForOpen clears media-specific state', () {
      store.apply(
        event(kind: PlayerEventKind.videoSize, width: 100, height: 100),
      );
      store.setTracks([
        const MediaTrack(
          id: 1,
          trackType: TrackType.video,
          language: 'en',
          label: 'Video',
          selected: true,
        ),
      ]);
      store.resetForOpen();

      expect(store.videoSize.value, Size.zero);
      expect(store.tracks.value, isEmpty);
      expect(store.error.value, isNull);
    });

    test('previewSeek updates position and optional buffering state', () {
      store.apply(
        event(kind: PlayerEventKind.stateChanged, state: PlayerState.playing),
      );
      store.previewSeek(const Duration(seconds: 30), showBuffering: true);
      expect(store.position.value, const Duration(seconds: 30));
      expect(store.state.value, PlayerState.buffering);
    });
  });
}
