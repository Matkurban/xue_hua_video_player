import 'dart:async';
import 'dart:typed_data';
import 'dart:ui' as ui;

/// Encodes tightly-packed or strided BGRA pixels to PNG bytes.
Future<Uint8List> bgraToPng({
  required Uint8List bgra,
  required int width,
  required int height,
  required int stride,
}) async {
  if (width <= 0 || height <= 0) {
    throw ArgumentError('invalid frame size ${width}x$height');
  }
  final expectedTight = width * 4 * height;
  late final Uint8List pixels;
  if (stride == width * 4 && bgra.lengthInBytes >= expectedTight) {
    pixels = bgra.lengthInBytes == expectedTight
        ? bgra
        : Uint8List.sublistView(bgra, 0, expectedTight);
  } else {
    pixels = Uint8List(expectedTight);
    for (var row = 0; row < height; row++) {
      final src = row * stride;
      final dst = row * width * 4;
      pixels.setRange(dst, dst + width * 4, bgra, src);
    }
  }

  final completer = Completer<ui.Image>();
  ui.decodeImageFromPixels(
    pixels,
    width,
    height,
    ui.PixelFormat.bgra8888,
    completer.complete,
  );
  final image = await completer.future;
  try {
    final byteData = await image.toByteData(format: ui.ImageByteFormat.png);
    if (byteData == null) {
      throw StateError('PNG encode failed');
    }
    return byteData.buffer.asUint8List(
      byteData.offsetInBytes,
      byteData.lengthInBytes,
    );
  } finally {
    image.dispose();
  }
}

/// Native capture result before PNG encoding.
class CapturedBgraFrame {
  const CapturedBgraFrame({
    required this.bytes,
    required this.width,
    required this.height,
    required this.stride,
  });

  final Uint8List bytes;
  final int width;
  final int height;
  final int stride;

  factory CapturedBgraFrame.fromMap(Map<String, Object?> map) {
    return CapturedBgraFrame(
      bytes: map['bytes']! as Uint8List,
      width: map['width']! as int,
      height: map['height']! as int,
      stride: map['stride']! as int,
    );
  }

  Future<Uint8List> toPng() =>
      bgraToPng(bgra: bytes, width: width, height: height, stride: stride);
}
