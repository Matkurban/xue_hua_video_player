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
            home: Scaffold(body: TextureVideoSurface(handle: handle)),
          ),
        );
        await tester.pumpAndSettle();
        expect(createTextureCount, 1);
        expect(disposeTextureCount, 0);

        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(body: TextureVideoSurface(handle: handle)),
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
          MaterialApp(home: Scaffold(body: _PlayerIdHost(initialPlayerId: 42))),
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

    testWidgets('view dispose does not release native texture', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      try {
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: TextureVideoSurface(
                handle: VideoSurfaceHandle.fromPlayerId(42),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(createTextureCount, 1);

        await tester.pumpWidget(
          const MaterialApp(home: Scaffold(body: SizedBox())),
        );
        await tester.pumpAndSettle();

        expect(
          disposeTextureCount,
          0,
          reason: 'texture lifetime follows the player, not the view',
        );
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });

    testWidgets('disposeNativePlayerTexture releases texture for player', (
      tester,
    ) async {
      await disposeNativePlayerTexture(42);
      expect(disposeTextureCount, 1);
    });

    testWidgets(
      'unsupported platform shows placeholder without createTexture',
      (tester) async {
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
      },
    );

    testWidgets(
      'Android syncTextureSize uses androidLayoutSize physical pixels',
      (tester) async {
        debugDefaultTargetPlatformOverride = TargetPlatform.android;
        MethodCall? syncCall;
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(textureChannel, (call) async {
              switch (call.method) {
                case 'createTexture':
                  createTextureCount++;
                  return 1;
                case 'disposeTexture':
                  disposeTextureCount++;
                  return null;
                case 'syncTextureSize':
                  syncCall = call;
                  return null;
                default:
                  return null;
              }
            });

        try {
          const dpr = 2.0;
          // Portrait viewport 400×800; contain 16:9 → ~400×225 (not 400×800).
          const viewport = Size(400, 800);
          const ratio = 16 / 9;
          final bufferLogical = applyBoxFit(
            BoxFit.contain,
            const Size(ratio, 1),
            viewport,
          ).destination;

          await tester.pumpWidget(
            MediaQuery(
              data: const MediaQueryData(devicePixelRatio: dpr),
              child: MaterialApp(
                home: Scaffold(
                  body: SizedBox(
                    width: viewport.width,
                    height: viewport.height,
                    child: FittedBox(
                      fit: BoxFit.contain,
                      child: SizedBox(
                        width: ratio,
                        height: 1,
                        child: TextureVideoSurface(
                          handle: VideoSurfaceHandle.fromPlayerId(7),
                          androidLayoutSize: bufferLogical,
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();

          expect(createTextureCount, 1);
          expect(syncCall, isNotNull);
          expect(syncCall!.method, 'syncTextureSize');
          final args = syncCall!.arguments! as Map;
          expect(args['playerId'], 7);
          final w = args['width'] as int;
          final h = args['height'] as int;
          expect(w, (bufferLogical.width * dpr).round());
          expect(h, (bufferLogical.height * dpr).round());
          expect(w, greaterThan(1));
          expect(h, greaterThan(1));
          // Must keep ~16:9 — not the portrait viewport aspect.
          expect(w / h, closeTo(ratio, 0.05));
          expect(h, lessThan((viewport.height * dpr).round()));
        } finally {
          debugDefaultTargetPlatformOverride = null;
        }
      },
    );

    testWidgets(
      'Android syncTextureSize falls back to MediaQuery when layout is zero',
      (tester) async {
        debugDefaultTargetPlatformOverride = TargetPlatform.android;
        MethodCall? syncCall;
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(textureChannel, (call) async {
              switch (call.method) {
                case 'createTexture':
                  createTextureCount++;
                  return 1;
                case 'disposeTexture':
                  disposeTextureCount++;
                  return null;
                case 'syncTextureSize':
                  syncCall = call;
                  return null;
                default:
                  return null;
              }
            });

        try {
          const dpr = 2.0;
          const screen = Size(400, 800);
          const ratio = 16 / 9;
          final expectedLogical = applyBoxFit(
            BoxFit.contain,
            const Size(ratio, 1),
            screen,
          ).destination;

          await tester.pumpWidget(
            MediaQuery(
              data: const MediaQueryData(size: screen, devicePixelRatio: dpr),
              child: MaterialApp(
                home: Scaffold(
                  body: TextureVideoSurface(
                    handle: VideoSurfaceHandle.fromPlayerId(7),
                    androidLayoutSize: Size.zero,
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();

          expect(createTextureCount, 1);
          expect(syncCall, isNotNull);
          final args = syncCall!.arguments! as Map;
          expect(args['width'], (expectedLogical.width * dpr).round());
          expect(args['height'], (expectedLogical.height * dpr).round());
        } finally {
          debugDefaultTargetPlatformOverride = null;
        }
      },
    );
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
