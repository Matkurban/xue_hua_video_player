import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/player/state_store.dart';
import 'package:xue_hua_video_player/src/rust/player_events.dart';
import 'package:xue_hua_video_player/src/xue_hua_player_controller.dart';

import 'support/fake_player_command_port.dart';
import 'support/player_event_fixtures.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('XueHuaPlayerController', () {
    late FakePlayerCommandPort port;
    late PlayerStateStore store;
    late XueHuaPlayerController controller;

    setUp(() {
      port = FakePlayerCommandPort();
      store = PlayerStateStore();
      controller = XueHuaPlayerController(port: port, store: store);
    });

    tearDown(() async {
      await controller.dispose();
    });

    test('initialize wires playerId and event stream', () async {
      await controller.initialize();
      expect(controller.initialized.value, isTrue);
      expect(controller.playerId.value, 42);
    });

    test('open resolves source and updates capabilities', () async {
      await controller.initialize();
      await controller.open(VideoSource.network('https://example.com/a.mp4'));

      expect(port.lastLoadedSource, isA<MediaSourceDto_Uri>());
      expect(
        (port.lastLoadedSource! as MediaSourceDto_Uri).field0,
        'https://example.com/a.mp4',
      );
      expect(controller.isSeekable.value, isTrue);
      expect(controller.supportsTracks.value, isFalse);
      expect(controller.supportsOrientation.value, isTrue);
    });

    test('open failure surfaces error on store', () async {
      port = FakePlayerCommandPort(failLoad: true);
      controller = XueHuaPlayerController(port: port, store: store);
      await controller.initialize();

      await controller.open(VideoSource.network('https://x.test/v'));

      expect(controller.error.value, contains('load failed'));
      expect(controller.state.value, PlayerState.error);
    });

    test('seek applies optimistic position before port call', () async {
      await controller.initialize();
      port.emit(PlayerEventFixtures.stateChanged(state: PlayerState.playing));

      await controller.seek(const Duration(seconds: 12));

      expect(controller.position.value, const Duration(seconds: 12));
      expect(port.lastSeekPosition, const Duration(seconds: 12));
    });

    test('seek failure after optimistic update sets error', () async {
      port = FakePlayerCommandPort(failSeek: true);
      controller = XueHuaPlayerController(port: port, store: store);
      await controller.initialize();

      await controller.seek(const Duration(seconds: 5));

      expect(controller.position.value, const Duration(seconds: 5));
      expect(controller.error.value, contains('seek failed'));
    });

    test('PlayerEventKind.error applies error state', () async {
      await controller.initialize();
      port.emit(PlayerEventFixtures.error(message: 'decode error'));
      await Future<void>.delayed(Duration.zero);

      expect(controller.error.value, 'decode error');
      expect(controller.state.value, PlayerState.error);
    });

    test('togglePlayPause pauses when playing', () async {
      await controller.initialize();
      store.apply(PlayerEventFixtures.stateChanged(state: PlayerState.playing));

      await controller.togglePlayPause();

      expect(port.pauseCallCount, 1);
      expect(port.playCallCount, 0);
    });

    test('togglePlayPause plays when not playing', () async {
      await controller.initialize();
      store.apply(PlayerEventFixtures.stateChanged(state: PlayerState.paused));

      await controller.togglePlayPause();

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
      controller = XueHuaPlayerController(port: port, store: store);
      await controller.initialize();

      port.emit(PlayerEventFixtures.tracksChanged());
      await Future<void>.delayed(Duration.zero);

      expect(port.getTracksCallCount, 1);
      expect(controller.tracks.value, hasLength(1));
      expect(controller.tracks.value.first.label, 'English');
    });

    test('setVolume applies optimistic update and forwards to port', () async {
      await controller.initialize();

      await controller.setVolume(0.4);

      expect(controller.volume.value, 0.4);
      expect(port.lastVolume, 0.4);
    });

    test('setMuted applies optimistic update and forwards to port', () async {
      await controller.initialize();

      await controller.setMuted(true);

      expect(controller.muted.value, isTrue);
      expect(port.lastMute, isTrue);
    });
  });
}
