import 'package:integration_test/integration_test.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/xue_hua_video_player.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => await XueHuaVideoPlayer.ensureReady());
}
