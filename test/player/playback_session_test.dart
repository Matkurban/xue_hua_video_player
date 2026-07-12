import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/enum/video_rotation.dart';
import 'package:xue_hua_video_player/src/model/video_source.dart';
import 'package:xue_hua_video_player/src/player/playback_session.dart';
import 'package:xue_hua_video_player/src/domain/player_events.dart';

import '../support/fake_player_command_port.dart';
import '../support/player_event_fixtures.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('PlaybackSession', () {
    late FakePlayerCommandPort port;
    late PlaybackSession session;
    const textureChannel = MethodChannel('xue_hua_video_player/texture');
    var disposeTextureCount = 0;

    setUp(() {
      port = FakePlayerCommandPort();
      session = PlaybackSession(port: port);
      disposeTextureCount = 0;
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(textureChannel, (call) async {
            if (call.method == 'disposeTexture') {
              disposeTextureCount++;
            }
            return null;
          });
    });

    tearDown(() async {
      await session.dispose();
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(textureChannel, null);
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

    test('initialize wires playerId and event stream', () async {
      await session.initialize();
      expect(session.initialized.value, isTrue);
      expect(session.playerId.value, 42);
    });

    test(
      'dispose releases native texture while playerId is still valid',
      () async {
        await session.initialize();
        expect(session.playerId.value, 42);

        await session.dispose();

        expect(disposeTextureCount, 1);
      },
    );

    test('supportsOrientation defaults false before open', () {
      expect(session.supportsOrientation.value, isFalse);
    });

    test('open resolves source and updates capabilities', () async {
      await session.initialize();
      await session.open(VideoSource.network('https://example.com/a.mp4'));

      expect(port.lastLoadedSource, isA<MediaSourceDto_Uri>());
      expect(
        (port.lastLoadedSource! as MediaSourceDto_Uri).field0,
        'https://example.com/a.mp4',
      );
      expect(session.isSeekable.value, isTrue);
      expect(session.supportsTracks.value, isFalse);
      expect(session.supportsOrientation.value, isTrue);
      expect(session.videoRotation.value, VideoRotation.deg0);
    });

    test('mediaGeneration increments on each open', () async {
      await session.initialize();
      expect(session.mediaGeneration.value, 0);

      await session.open(VideoSource.network('https://example.com/a.mp4'));
      expect(session.mediaGeneration.value, 1);

      await session.open(VideoSource.network('https://example.com/b.mp4'));
      expect(session.mediaGeneration.value, 2);
    });

    test('open failure surfaces error state', () async {
      port = FakePlayerCommandPort(failLoad: true);
      session = PlaybackSession(port: port);
      await session.initialize();

      await session.open(VideoSource.network('https://x.test/v'));

      expect(session.error.value, contains('load failed'));
      expect(session.state.value, PlayerState.error);
      expect(session.bufferingPercent.value, 100);
    });

    test('seek applies optimistic position before port call', () async {
      await session.initialize();
      port.emit(PlayerEventFixtures.stateChanged(state: PlayerState.playing));
      await Future<void>.delayed(Duration.zero);

      await session.seek(const Duration(seconds: 12));

      expect(session.position.value, const Duration(seconds: 12));
      expect(port.lastSeekPosition, const Duration(seconds: 12));
    });

    test('seek failure after optimistic update sets error', () async {
      port = FakePlayerCommandPort(failSeek: true);
      session = PlaybackSession(port: port);
      await session.initialize();

      await session.seek(const Duration(seconds: 5));

      expect(session.position.value, const Duration(seconds: 5));
      expect(session.error.value, contains('seek failed'));
    });

    test('PlayerEventKind.error applies error state', () async {
      await session.initialize();
      port.emit(PlayerEventFixtures.error(message: 'decode error'));
      await Future<void>.delayed(Duration.zero);

      expect(session.error.value, 'decode error');
      expect(session.state.value, PlayerState.error);
    });

    test('togglePlayPause pauses when playing', () async {
      await session.initialize();
      port.emit(PlayerEventFixtures.stateChanged(state: PlayerState.playing));
      await Future<void>.delayed(Duration.zero);

      await session.togglePlayPause();

      expect(port.pauseCallCount, 1);
      expect(port.playCallCount, 0);
    });

    test('togglePlayPause plays when not playing', () async {
      await session.initialize();
      port.emit(PlayerEventFixtures.stateChanged(state: PlayerState.paused));
      await Future<void>.delayed(Duration.zero);

      await session.togglePlayPause();

      expect(port.playCallCount, 1);
      expect(port.pauseCallCount, 0);
    });

    test('tracksChanged event refreshes tracks from port', () async {
      port = FakePlayerCommandPort(
        tracksToReturn: const [
          MediaTrack(
            id: 1,
            trackType: TrackType.audio,
            language: 'en',
            label: 'English',
            selected: true,
          ),
        ],
      );
      session = PlaybackSession(port: port);
      await session.initialize();

      port.emit(PlayerEventFixtures.tracksChanged());
      await Future<void>.delayed(Duration.zero);

      expect(port.getTracksCallCount, 1);
      expect(session.tracks.value, hasLength(1));
      expect(session.tracks.value.first.label, 'English');
    });

    test('setVolume applies optimistic update and forwards to port', () async {
      await session.initialize();

      await session.setVolume(0.4);

      expect(session.volume.value, 0.4);
      expect(port.lastVolume, 0.4);
    });

    test('setMuted applies optimistic update and forwards to port', () async {
      await session.initialize();

      await session.setMuted(true);

      expect(session.muted.value, isTrue);
      expect(port.lastMute, isTrue);
    });

    test(
      'stateChanged updates state without clobbering buffering percent',
      () async {
        await session.initialize();
        port.emit(event(kind: PlayerEventKind.buffering, bufferingPercent: 42));
        await Future<void>.delayed(Duration.zero);
        expect(session.bufferingPercent.value, 42);

        port.emit(
          event(kind: PlayerEventKind.stateChanged, state: PlayerState.playing),
        );
        await Future<void>.delayed(Duration.zero);
        expect(session.state.value, PlayerState.playing);
        expect(session.bufferingPercent.value, 42);
      },
    );

    test('eos marks completed and snaps position to duration', () async {
      await session.initialize();
      port.emit(event(kind: PlayerEventKind.durationChanged, durationMs: 5000));
      await Future<void>.delayed(Duration.zero);
      port.emit(event(kind: PlayerEventKind.positionChanged, positionMs: 4800));
      await Future<void>.delayed(Duration.zero);
      port.emit(event(kind: PlayerEventKind.eos));
      await Future<void>.delayed(Duration.zero);

      expect(session.state.value, PlayerState.completed);
      expect(session.position.value, const Duration(milliseconds: 5000));
      expect(session.isCompleted.value, isTrue);
    });

    test('metadataChanged drives aspectRatio computed signal', () async {
      await session.initialize();
      port.emit(
        event(kind: PlayerEventKind.metadataChanged, width: 1920, height: 1080),
      );
      await Future<void>.delayed(Duration.zero);
      expect(session.aspectRatio.value, closeTo(16 / 9, 0.001));
    });

    test('videoSize fallback when metadata lacks display aspect', () async {
      await session.initialize();
      port.emit(
        event(kind: PlayerEventKind.videoSize, width: 640, height: 480),
      );
      await Future<void>.delayed(Duration.zero);
      expect(session.aspectRatio.value, closeTo(4 / 3, 0.001));
    });

    test('portrait videoSize drives aspectRatio below 1', () async {
      await session.initialize();
      port.emit(
        event(kind: PlayerEventKind.videoSize, width: 1080, height: 1920),
      );
      await Future<void>.delayed(Duration.zero);
      expect(session.videoSize.value, const Size(1080, 1920));
      expect(session.aspectRatio.value, closeTo(9 / 16, 0.001));
    });

    test('aspectRatio follows post-orient videoSize after rotation', () async {
      await session.initialize();
      port.emit(
        event(kind: PlayerEventKind.videoSize, width: 1920, height: 1080),
      );
      await Future<void>.delayed(Duration.zero);
      expect(session.aspectRatio.value, closeTo(16 / 9, 0.001));

      // Native videoflip/glvideoflip emit swapped size; Dart does not invert.
      await session.setVideoRotation(VideoRotation.deg90);
      expect(session.aspectRatio.value, closeTo(16 / 9, 0.001));

      port.emit(
        event(kind: PlayerEventKind.videoSize, width: 1080, height: 1920),
      );
      await Future<void>.delayed(Duration.zero);
      expect(session.aspectRatio.value, closeTo(9 / 16, 0.001));
    });

    test(
      'metadataChanged does not clobber isSeekable from capabilities',
      () async {
        port = FakePlayerCommandPort(seekable: false);
        session = PlaybackSession(port: port);
        await session.initialize();
        await session.open(VideoSource.network('https://example.com/asset'));
        expect(session.isSeekable.value, isFalse);

        port.emit(
          event(
            kind: PlayerEventKind.metadataChanged,
            width: 1920,
            height: 1080,
          ),
        );
        await Future<void>.delayed(Duration.zero);

        expect(session.isSeekable.value, isFalse);
      },
    );

    test('open resets transport state from previous playback', () async {
      await session.initialize();
      port.emit(PlayerEventFixtures.stateChanged(state: PlayerState.playing));
      port.emit(
        event(
          kind: PlayerEventKind.positionChanged,
          positionMs: 9000,
          durationMs: 10000,
        ),
      );
      port.emit(
        event(kind: PlayerEventKind.durationChanged, durationMs: 10000),
      );
      await Future<void>.delayed(Duration.zero);

      expect(session.state.value, PlayerState.playing);
      expect(session.position.value, const Duration(milliseconds: 9000));

      await session.open(VideoSource.network('https://example.com/b.mp4'));

      expect(session.state.value, PlayerState.buffering);
      expect(session.bufferingPercent.value, 0);
      expect(session.position.value, Duration.zero);
      expect(session.duration.value, Duration.zero);
    });

    test('open clears media-specific state', () async {
      await session.initialize();
      port.emit(
        event(kind: PlayerEventKind.videoSize, width: 100, height: 100),
      );
      await Future<void>.delayed(Duration.zero);
      port.emit(PlayerEventFixtures.tracksChanged());
      await Future<void>.delayed(Duration.zero);

      await session.open(VideoSource.network('https://example.com/b.mp4'));

      expect(session.videoSize.value, Size.zero);
      expect(session.tracks.value, isEmpty);
      expect(session.error.value, isNull);
    });

    test('seek while playing does not set buffering optimistically', () async {
      await session.initialize();
      port.emit(PlayerEventFixtures.stateChanged(state: PlayerState.playing));
      await Future<void>.delayed(Duration.zero);

      await session.seek(const Duration(seconds: 30));

      expect(session.position.value, const Duration(seconds: 30));
      expect(session.state.value, PlayerState.playing);
    });

    test('open resets video rotation after prior rotation', () async {
      await session.initialize();
      await session.setVideoRotation(VideoRotation.deg90);
      expect(port.lastVideoRotationDegrees, 90);

      await session.open(VideoSource.network('https://example.com/b.mp4'));

      expect(session.videoRotation.value, VideoRotation.deg0);
      expect(port.lastVideoRotationDegrees, 0);
    });

    test('setVideoRotation updates videoRotation signal', () async {
      await session.initialize();
      const rotation = VideoRotation.deg90;

      await session.setVideoRotation(rotation);

      expect(session.videoRotation.value, rotation);
      expect(port.lastVideoRotationDegrees, 90);
    });

    // Regression: a macOS overlay-apply deadlock (overlay_sink locked on a
    // background thread while osxvideosink marshalled set_window_handle to the
    // main thread) froze the whole Dart isolate, so no PlayerEvent ever reached
    // the session. Controls looked dead: `isPlaying` stayed false, so
    // `togglePlayPause` only ever issued play, the progress bar never advanced,
    // and the slider stayed disabled. This guards the end-to-end contract that
    // once engine events flow, the controls become live and controllable.
    group('controls stay live when engine playback events flow', () {
      test(
        'playing + duration + position events drive controls model',
        () async {
          await session.initialize();
          await session.open(VideoSource.network('https://example.com/a.mp4'));

          // Before any playback event the player is not "playing".
          expect(session.isPlaying.value, isFalse);

          port.emit(
            PlayerEventFixtures.stateChanged(state: PlayerState.playing),
          );
          port.emit(
            event(kind: PlayerEventKind.durationChanged, durationMs: 10000),
          );
          port.emit(
            event(kind: PlayerEventKind.positionChanged, positionMs: 3000),
          );
          await Future<void>.delayed(Duration.zero);

          // Controls now reflect a live, seekable player.
          expect(session.isPlaying.value, isTrue);
          expect(session.duration.value, const Duration(seconds: 10));
          expect(session.position.value, const Duration(seconds: 3));

          // A later position event advances the progress (not frozen at 0).
          port.emit(
            event(kind: PlayerEventKind.positionChanged, positionMs: 4000),
          );
          await Future<void>.delayed(Duration.zero);
          expect(session.position.value, const Duration(seconds: 4));
        },
      );

      test(
        'togglePlayPause issues pause once a playing event has arrived',
        () async {
          await session.initialize();
          await session.open(VideoSource.network('https://example.com/a.mp4'));

          port.emit(
            PlayerEventFixtures.stateChanged(state: PlayerState.playing),
          );
          await Future<void>.delayed(Duration.zero);

          await session.togglePlayPause();

          // The bug made isPlaying stuck false, so toggle always called play;
          // with events flowing it must pause instead ("按暂停视频不停" fix).
          expect(port.pauseCallCount, 1);
          expect(port.playCallCount, 0);
        },
      );
    });
  });
}
