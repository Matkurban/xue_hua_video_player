import '../enum/video_source_type.dart';
import '../model/video_source.dart';
import '../rust/player_events.dart';

/// Resolves a Dart [VideoSource] into a Rust [MediaSourceDto] at the Dart/Rust seam.
class MediaSourceResolver {
  const MediaSourceResolver();

  MediaSourceDto resolve(VideoSource source) {
    switch (source.type) {
      case VideoSourceType.asset:
        return MediaSourceDto.flutterAsset(source.uri.trim());
      case VideoSourceType.network:
      case VideoSourceType.file:
        return MediaSourceDto.uri(_resolveGstUri(source));
    }
  }

  static String _resolveGstUri(VideoSource source) {
    switch (source.type) {
      case VideoSourceType.network:
        return source.uri.trim();
      case VideoSourceType.file:
        final trimmed = source.uri.trim();
        final parsed = Uri.tryParse(trimmed);
        if (parsed != null && parsed.hasScheme) {
          return trimmed;
        }
        return Uri.file(trimmed).toString();
      case VideoSourceType.asset:
        return source.uri.trim();
    }
  }
}
