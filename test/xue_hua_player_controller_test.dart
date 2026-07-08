import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/player/playback_session.dart';
import 'package:xue_hua_video_player/src/xue_hua_player_controller.dart';

import 'support/fake_player_command_port.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('XueHuaPlayerController', () {
    test('delegates initialize to PlaybackSession', () async {
      final port = FakePlayerCommandPort();
      final session = PlaybackSession(port: port);
      final controller = XueHuaPlayerController(session: session);

      await controller.initialize();

      expect(controller.initialized.value, isTrue);
      expect(controller.playerId.value, 42);

      await controller.dispose();
    });

    test('delegates dispose to PlaybackSession', () async {
      final port = FakePlayerCommandPort();
      final session = PlaybackSession(port: port);
      final controller = XueHuaPlayerController(session: session);

      await controller.initialize();
      await controller.dispose();

      expect(port.playerId, isNull);
    });
  });
}
