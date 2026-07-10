import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/media/media_source_resolver.dart';
import 'package:xue_hua_video_player/src/model/video_source.dart';
import 'package:xue_hua_video_player/src/domain/player_events.dart';

void main() {
  const resolver = MediaSourceResolver();

  group('MediaSourceResolver', () {
    test('network source passes URI through trimmed', () {
      final dto = resolver.resolve(
        VideoSource.network('  https://example.com/v.mp4  '),
      );
      expect(dto, isA<MediaSourceDto_Uri>());
      expect((dto as MediaSourceDto_Uri).field0, 'https://example.com/v.mp4');
    });

    test('asset source maps to flutterAsset', () {
      final dto = resolver.resolve(
        const VideoSource.asset(' videos/demo.mp4 '),
      );
      expect(dto, isA<MediaSourceDto_FlutterAsset>());
      expect((dto as MediaSourceDto_FlutterAsset).field0, 'videos/demo.mp4');
    });

    test('file path without scheme becomes file URI', () {
      final dto = resolver.resolve(VideoSource.file('/tmp/video.mp4'));
      expect(dto, isA<MediaSourceDto_Uri>());
      expect((dto as MediaSourceDto_Uri).field0, startsWith('file://'));
    });

    test('file URI with scheme is preserved', () {
      const uri = 'file:///tmp/video.mp4';
      final dto = resolver.resolve(VideoSource.file(uri));
      expect(dto, isA<MediaSourceDto_Uri>());
      expect((dto as MediaSourceDto_Uri).field0, uri);
    });
  });
}
