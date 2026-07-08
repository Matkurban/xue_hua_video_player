import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/surface/texture_surface.dart';
import 'package:xue_hua_video_player/src/surface/video_surface_handle.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('TextureVideoSurface', () {
    const textureChannel = MethodChannel('xue_hua_video_player/texture');

    var createTextureCount = 0;
    var disposeTextureCount = 0;

    setUp(() {
      createTextureCount = 0;
      disposeTextureCount = 0;
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(textureChannel, (call) async {
        switch (call.method) {
          case 'createTexture':
            createTextureCount++;
            return 1;
          case 'disposeTexture':
            disposeTextureCount++;
            return null;
          default:
            return null;
        }
      });
    });

    tearDown(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(textureChannel, null);
    });

    testWidgets('didUpdateWidget keeps texture when playerId unchanged', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      try {
        final handle = VideoSurfaceHandle.fromPlayerId(42);
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: TextureVideoSurface(handle: handle),
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(createTextureCount, 1);
        expect(disposeTextureCount, 0);

        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: TextureVideoSurface(handle: handle),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(createTextureCount, 1);
        expect(disposeTextureCount, 0);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('didUpdateWidget recreates texture when playerId changes', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      try {
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: _PlayerIdHost(initialPlayerId: 42),
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(createTextureCount, 1);

        await tester.tap(find.byType(_PlayerIdHost));
        await tester.pumpAndSettle();

        expect(disposeTextureCount, 1);
        expect(createTextureCount, 2);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('unsupported platform shows placeholder without createTexture', (
      tester,
    ) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: TextureVideoSurface(
              handle: const VideoSurfaceHandle(
                playerId: 42,
                kind: VideoSurfaceKind.unsupported,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(createTextureCount, 0);
      expect(disposeTextureCount, 0);
      expect(find.textContaining('Video not supported'), findsOneWidget);
    });
  });
}

/// 测试用宿主：点击后在同一 Element 树内切换 playerId / Test host that switches playerId in-place on tap.
class _PlayerIdHost extends StatefulWidget {
  const _PlayerIdHost({required this.initialPlayerId});

  final int initialPlayerId;

  @override
  State<_PlayerIdHost> createState() => _PlayerIdHostState();
}

class _PlayerIdHostState extends State<_PlayerIdHost> {
  late int _playerId;

  @override
  void initState() {
    super.initState();
    _playerId = widget.initialPlayerId;
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: () => setState(() => _playerId = 99),
      behavior: HitTestBehavior.opaque,
      child: TextureVideoSurface(
        handle: VideoSurfaceHandle.fromPlayerId(_playerId),
      ),
    );
  }
}
