import Cocoa
import FlutterMacOS

@_silgen_name("xhvp_ffi_retain_symbols")
func xhvp_ffi_retain_symbols()

public class XueHuaVideoPlayerPlugin: NSObject, FlutterPlugin {
  public static let textureChannelName = "xue_hua_video_player/texture"

  private let textures: FlutterTextureRegistry
  private var videoTextures: [Int64: XueHuaVideoTexture] = [:]
  private let lock = NSLock()

  init(textures: FlutterTextureRegistry) {
    self.textures = textures
    super.init()
  }

  public static func register(with registrar: FlutterPluginRegistrar) {
    // Keep Dart FFI ABI symbols alive for DynamicLibrary.process() / dlsym.
    xhvp_ffi_retain_symbols()
    let instance = XueHuaVideoPlayerPlugin(textures: registrar.textures)
    let channel = FlutterMethodChannel(
      name: textureChannelName, binaryMessenger: registrar.messenger)
    registrar.addMethodCallDelegate(instance, channel: channel)
  }

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    let args = call.arguments as? [String: Any]
    let playerId = (args?["playerId"] as? NSNumber)?.int64Value ?? 0
    switch call.method {
    case "createTexture":
      lock.lock()
      defer { lock.unlock() }
      if let existing = videoTextures[playerId] {
        result(NSNumber(value: existing.textureId))
        return
      }
      let texture = XueHuaVideoTexture(playerId: playerId, registry: textures)
      videoTextures[playerId] = texture
      result(NSNumber(value: texture.textureId))
    case "disposeTexture":
      lock.lock()
      defer { lock.unlock() }
      if let texture = videoTextures.removeValue(forKey: playerId) {
        texture.dispose()
      }
      result(nil)
    default:
      result(FlutterMethodNotImplemented)
    }
  }
}
