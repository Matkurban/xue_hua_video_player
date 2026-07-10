import 'package:xue_hua_video_player/src/domain/player_events.dart';

/// Builds [PlayerEvent] values for unit/widget tests without boilerplate.
abstract final class PlayerEventFixtures {
  static PlayerEvent stateChanged({
    PlayerState state = PlayerState.playing,
    int positionMs = 0,
    int durationMs = 0,
    int bufferingPercent = 100,
    bool isSeekable = true,
  }) {
    return PlayerEvent(
      kind: PlayerEventKind.stateChanged,
      positionMs: positionMs,
      durationMs: durationMs,
      width: 0,
      height: 0,
      bufferingPercent: bufferingPercent,
      state: state,
      message: '',
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

  static PlayerEvent error({String message = 'error'}) {
    return PlayerEvent(
      kind: PlayerEventKind.error,
      positionMs: 0,
      durationMs: 0,
      width: 0,
      height: 0,
      bufferingPercent: 0,
      state: PlayerState.error,
      message: message,
      fps: 0,
      pixelAspectWidth: 1,
      pixelAspectHeight: 1,
      displayAspectWidth: 1,
      displayAspectHeight: 1,
      interlaced: false,
      colorMatrix: '',
      colorRange: '',
      hdrFormat: '',
      isSeekable: true,
    );
  }

  static PlayerEvent tracksChanged() {
    return PlayerEvent(
      kind: PlayerEventKind.tracksChanged,
      positionMs: 0,
      durationMs: 0,
      width: 0,
      height: 0,
      bufferingPercent: 0,
      state: PlayerState.playing,
      message: '',
      fps: 0,
      pixelAspectWidth: 1,
      pixelAspectHeight: 1,
      displayAspectWidth: 1,
      displayAspectHeight: 1,
      interlaced: false,
      colorMatrix: '',
      colorRange: '',
      hdrFormat: '',
      isSeekable: true,
    );
  }
}
