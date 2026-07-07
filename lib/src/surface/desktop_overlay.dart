import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'desktop_overlay_bounds.dart';
import 'video_surface_handle.dart';

/// Native desktop overlay IO (Windows / Linux MethodChannel).
abstract class DesktopOverlayClient {
  Future<void> attach(int playerId);
  Future<void> detach(int playerId);
  Future<void> setBounds(int playerId, DesktopOverlayBounds bounds);
}

/// Production client for `xue_hua_video_player/desktop_overlay`.
class MethodChannelDesktopOverlayClient implements DesktopOverlayClient {
  MethodChannelDesktopOverlayClient({MethodChannel? channel})
    : _channel = channel ?? _defaultChannel;

  static const MethodChannel _defaultChannel = MethodChannel(
    'xue_hua_video_player/desktop_overlay',
  );

  final MethodChannel _channel;

  @override
  Future<void> attach(int playerId) {
    return _channel.invokeMethod<void>('attach', <String, dynamic>{
      'playerId': playerId,
    });
  }

  @override
  Future<void> detach(int playerId) {
    return _channel.invokeMethod<void>('detach', <String, dynamic>{
      'playerId': playerId,
    });
  }

  @override
  Future<void> setBounds(int playerId, DesktopOverlayBounds bounds) {
    return _channel.invokeMethod<void>(
      'setBounds',
      bounds.toChannelArgs(playerId),
    );
  }
}

/// Positions a native overlay window for GStreamer on desktop platforms where
/// Flutter PlatformView embedding is not yet available in the framework.
class DesktopVideoOverlay extends StatefulWidget {
  const DesktopVideoOverlay({super.key, required this.handle, this._client});

  final VideoSurfaceHandle handle;
  final DesktopOverlayClient? _client;

  @visibleForTesting
  DesktopOverlayClient get client =>
      _client ?? MethodChannelDesktopOverlayClient();

  @override
  State<DesktopVideoOverlay> createState() => _DesktopVideoOverlayState();
}

class _DesktopVideoOverlayState extends State<DesktopVideoOverlay>
    with WidgetsBindingObserver {
  late final DesktopOverlayClient _client;

  @override
  void initState() {
    super.initState();
    _client = widget.client;
    WidgetsBinding.instance.addObserver(this);
    _client.attach(widget.handle.playerId);
    WidgetsBinding.instance.addPostFrameCallback((_) => _syncBounds());
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _client.detach(widget.handle.playerId);
    super.dispose();
  }

  @override
  void didChangeMetrics() {
    WidgetsBinding.instance.addPostFrameCallback((_) => _syncBounds());
  }

  void _syncBounds() {
    if (!mounted) return;
    final box = context.findRenderObject();
    if (box is! RenderBox) return;
    final bounds = DesktopOverlayBounds.fromRenderBox(box);
    if (bounds == null) return;
    _client.setBounds(widget.handle.playerId, bounds);
  }

  @override
  Widget build(BuildContext context) {
    WidgetsBinding.instance.addPostFrameCallback((_) => _syncBounds());
    return const SizedBox.expand();
  }
}
