import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:xue_hua_video_player/src/surface/desktop_overlay_bounds.dart';

void main() {
  group('DesktopOverlayBounds.fromRenderBox', () {
    const boxKey = Key('bounds-box');

    testWidgets('returns global offset and size', (tester) async {
      await tester.pumpWidget(
        const Directionality(
          textDirection: TextDirection.ltr,
          child: Center(
            child: SizedBox(
              key: boxKey,
              width: 120,
              height: 80,
              child: ColoredBox(color: Color(0xFF000000)),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      final box = tester.renderObject(find.byKey(boxKey)) as RenderBox;
      final bounds = DesktopOverlayBounds.fromRenderBox(box);

      expect(bounds, isNotNull);
      expect(bounds!.width, 120);
      expect(bounds.height, 80);
      expect(bounds.x, isNonNegative);
      expect(bounds.y, isNonNegative);
    });

    test('toChannelArgs includes playerId and geometry', () {
      const bounds = DesktopOverlayBounds(x: 1, y: 2, width: 3, height: 4);
      expect(bounds.toChannelArgs(9), <String, dynamic>{
        'playerId': 9,
        'x': 1.0,
        'y': 2.0,
        'width': 3.0,
        'height': 4.0,
      });
    });
  });
}
