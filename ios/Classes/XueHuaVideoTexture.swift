import CoreVideo
import Flutter
import Foundation

// Rust C-ABI frame bridge (see rust/src/platform/texture.rs).
@_silgen_name("xhvp_texture_register")
func xhvp_texture_register(
  _ playerId: Int64,
  _ ctx: UnsafeMutableRawPointer?,
  _ onFrame: @convention(c) (UnsafeMutableRawPointer?) -> Void
)

@_silgen_name("xhvp_texture_unregister")
func xhvp_texture_unregister(_ playerId: Int64)

@_silgen_name("xhvp_texture_frame_info")
func xhvp_texture_frame_info(
  _ playerId: Int64,
  _ outWidth: UnsafeMutablePointer<Int32>?,
  _ outHeight: UnsafeMutablePointer<Int32>?,
  _ outStride: UnsafeMutablePointer<Int32>?,
  _ outBytes: UnsafeMutablePointer<UInt32>?
) -> Bool

@_silgen_name("xhvp_texture_copy_latest")
func xhvp_texture_copy_latest(
  _ playerId: Int64,
  _ dst: UnsafeMutablePointer<UInt8>?,
  _ dstLen: UInt32,
  _ outWidth: UnsafeMutablePointer<Int32>?,
  _ outHeight: UnsafeMutablePointer<Int32>?,
  _ outStride: UnsafeMutablePointer<Int32>?
) -> Bool

/// C trampoline invoked by Rust on the GStreamer streaming thread when a new
/// frame is available. `ctx` is the unretained `XueHuaVideoTexture`.
private func xhvpTextureOnFrame(_ ctx: UnsafeMutableRawPointer?) {
  guard let ctx = ctx else { return }
  let texture = Unmanaged<XueHuaVideoTexture>.fromOpaque(ctx).takeUnretainedValue()
  texture.onFrameAvailable()
}

/// Flutter external texture backed by GStreamer BGRA frames pulled from Rust and
/// wrapped in an IOSurface-backed `CVPixelBuffer` (renders correctly on device).
final class XueHuaVideoTexture: NSObject, FlutterTexture {
  private let playerId: Int64
  private weak var registry: FlutterTextureRegistry?
  private(set) var textureId: Int64 = 0

  // Reused staging buffer (Rust frame incl. row stride) and pixel buffer.
  private var staging = [UInt8]()
  private var pixelBuffer: CVPixelBuffer?
  private var pbWidth: Int32 = 0
  private var pbHeight: Int32 = 0
  private let lock = NSLock()

  init(playerId: Int64, registry: FlutterTextureRegistry) {
    self.playerId = playerId
    self.registry = registry
    super.init()
    textureId = registry.register(self)
    let ctx = Unmanaged.passUnretained(self).toOpaque()
    xhvp_texture_register(playerId, ctx, xhvpTextureOnFrame)
  }

  /// Called from Rust (streaming thread). `textureFrameAvailable` is thread-safe.
  func onFrameAvailable() {
    registry?.textureFrameAvailable(textureId)
  }

  func dispose() {
    xhvp_texture_unregister(playerId)
    if let registry = registry, textureId != 0 {
      registry.unregisterTexture(textureId)
    }
    lock.lock()
    pixelBuffer = nil
    staging = []
    lock.unlock()
  }

  // MARK: FlutterTexture

  func copyPixelBuffer() -> Unmanaged<CVPixelBuffer>? {
    lock.lock()
    defer { lock.unlock() }

    var width: Int32 = 0
    var height: Int32 = 0
    var stride: Int32 = 0
    var bytes: UInt32 = 0
    guard
      xhvp_texture_frame_info(playerId, &width, &height, &stride, &bytes),
      width > 0, height > 0, bytes > 0
    else {
      return nil
    }

    if staging.count < Int(bytes) {
      staging = [UInt8](repeating: 0, count: Int(bytes))
    }
    var copiedW: Int32 = 0
    var copiedH: Int32 = 0
    var copiedStride: Int32 = 0
    let ok = staging.withUnsafeMutableBufferPointer { buf in
      xhvp_texture_copy_latest(
        playerId, buf.baseAddress, bytes, &copiedW, &copiedH, &copiedStride)
    }
    guard ok, copiedW > 0, copiedH > 0 else { return nil }

    guard let pb = ensurePixelBuffer(width: copiedW, height: copiedH) else { return nil }

    CVPixelBufferLockBaseAddress(pb, [])
    if let base = CVPixelBufferGetBaseAddress(pb) {
      let dstStride = CVPixelBufferGetBytesPerRow(pb)
      let srcStride = Int(copiedStride)
      let rowBytes = min(Int(copiedW) * 4, min(srcStride, dstStride))
      staging.withUnsafeBufferPointer { src in
        guard let srcBase = src.baseAddress else { return }
        for row in 0..<Int(copiedH) {
          memcpy(
            base.advanced(by: row * dstStride),
            srcBase.advanced(by: row * srcStride),
            rowBytes)
        }
      }
    }
    CVPixelBufferUnlockBaseAddress(pb, [])
    return Unmanaged.passRetained(pb)
  }

  private func ensurePixelBuffer(width: Int32, height: Int32) -> CVPixelBuffer? {
    if let pb = pixelBuffer, pbWidth == width, pbHeight == height {
      return pb
    }
    let attrs: [String: Any] = [
      kCVPixelBufferIOSurfacePropertiesKey as String: [String: Any](),
      kCVPixelBufferPixelFormatTypeKey as String: Int(kCVPixelFormatType_32BGRA),
      kCVPixelBufferMetalCompatibilityKey as String: true,
    ]
    var pb: CVPixelBuffer?
    let status = CVPixelBufferCreate(
      kCFAllocatorDefault, Int(width), Int(height),
      kCVPixelFormatType_32BGRA, attrs as CFDictionary, &pb)
    guard status == kCVReturnSuccess, let created = pb else {
      return nil
    }
    pixelBuffer = created
    pbWidth = width
    pbHeight = height
    return created
  }
}
