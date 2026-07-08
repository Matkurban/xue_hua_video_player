import 'package:flutter/widgets.dart';

import 'surface/build_video_surface.dart';
import 'surface/video_surface_handle.dart';

/// Builds the platform-appropriate video surface for [playerId].
///
/// Prefer [XueHuaVideoView] for full playback UI. This entry point remains for
/// custom layouts documented in README.
Widget buildXueHuaVideoPlatformView({required int playerId}) {
  return buildVideoSurface(VideoSurfaceHandle.fromPlayerId(playerId));
}
