// ignore_for_file: non_constant_identifier_names

import 'dart:async';
import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';

import '../domain/player_events.dart';
import 'xhvp_bindings.dart';
import 'xhvp_library.dart';

/// Bridges native [XhvpEventCallback] into a Dart [Stream] of [PlayerEvent].
class XhvpEventPump {
  XhvpEventPump(this._playerId);

  final int _playerId;
  final StreamController<PlayerEvent> _controller =
      StreamController<PlayerEvent>.broadcast();

  NativeCallable<XhvpEventCallbackFunction>? _callable;

  Stream<PlayerEvent> get stream => _controller.stream;

  void start() {
    if (_callable != null) {
      return;
    }
    _callable = NativeCallable<XhvpEventCallbackFunction>.listener(_onNative);
    XhvpLibrary.instance.bindings.xhvp_player_set_event_callback(
      _playerId,
      nullptr,
      _callable!.nativeFunction,
    );
  }

  /// Safe C string → Dart; never throws on freed/malformed pointers.
  static String _cString(Pointer<Char> ptr) {
    if (ptr == nullptr) {
      return '';
    }
    try {
      return ptr.cast<Utf8>().toDartString();
    } on FormatException {
      try {
        final bytes = ptr.cast<Uint8>();
        var len = 0;
        while (len < 512 && bytes[len] != 0) {
          len++;
        }
        return utf8.decode(bytes.asTypedList(len), allowMalformed: true);
      } catch (_) {
        return '';
      }
    } catch (_) {
      return '';
    }
  }

  void _onNative(
    Pointer<Void> ctx,
    int kind,
    int position_ms,
    int duration_ms,
    int width,
    int height,
    int buffering_percent,
    int state,
    Pointer<Char> message,
    double fps,
    int par_n,
    int par_d,
    int dar_n,
    int dar_d,
    bool interlaced,
    Pointer<Char> color_matrix,
    Pointer<Char> color_range,
    Pointer<Char> hdr_format,
    bool is_seekable,
  ) {
    final positionMs = position_ms;
    final durationMs = duration_ms;
    final bufferingPercent = buffering_percent;
    final parN = par_n;
    final parD = par_d;
    final darN = dar_n;
    final darD = dar_d;
    final isSeekable = is_seekable;
    if (_controller.isClosed) {
      return;
    }
    final kinds = PlayerEventKind.values;
    final states = PlayerState.values;
    final event = PlayerEvent(
      kind: kind >= 0 && kind < kinds.length
          ? kinds[kind]
          : PlayerEventKind.error,
      positionMs: positionMs,
      durationMs: durationMs,
      width: width,
      height: height,
      bufferingPercent: bufferingPercent,
      state: state >= 0 && state < states.length
          ? states[state]
          : PlayerState.error,
      message: _cString(message),
      fps: fps,
      pixelAspectWidth: parN,
      pixelAspectHeight: parD,
      displayAspectWidth: darN,
      displayAspectHeight: darD,
      interlaced: interlaced,
      colorMatrix: _cString(color_matrix),
      colorRange: _cString(color_range),
      hdrFormat: _cString(hdr_format),
      isSeekable: isSeekable,
    );
    _controller.add(event);
  }

  Future<void> dispose() async {
    // Clear callback before closing the NativeCallable.
    XhvpLibrary.instance.bindings.xhvp_player_set_event_callback(
      _playerId,
      nullptr,
      nullptr,
    );
    _callable?.close();
    _callable = null;
    if (!_controller.isClosed) {
      await _controller.close();
    }
  }
}
