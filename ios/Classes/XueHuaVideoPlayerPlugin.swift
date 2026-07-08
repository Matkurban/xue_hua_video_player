import Flutter
import UIKit

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
    if let assetsDir = flutterAssetsDirectory() {
      xhvp_set_flutter_assets_dir(assetsDir)
    }
    let instance = XueHuaVideoPlayerPlugin(textures: registrar.textures())
    let channel = FlutterMethodChannel(
      name: textureChannelName, binaryMessenger: registrar.messenger())
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

  private static func flutterAssetsDirectory() -> String? {
    let bundle = Bundle.main
    var candidates: [String] = []
    if let frameworks = bundle.privateFrameworksPath {
      candidates.append(
        (frameworks as NSString).appendingPathComponent("App.framework/flutter_assets")
      )
    }
    if let resource = bundle.path(
      forResource: "flutter_assets",
      ofType: nil,
      inDirectory: "Frameworks/App.framework"
    ) {
      candidates.append(resource)
    }
    for path in candidates where FileManager.default.fileExists(atPath: path) {
      return path
    }
    return nil
  }
}

@_silgen_name("xhvp_set_flutter_assets_dir")
func xhvp_set_flutter_assets_dir(_ path: UnsafePointer<CChar>)
