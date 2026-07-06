import Flutter
import UIKit

public class XueHuaVideoPlayerPlugin: NSObject, FlutterPlugin {
  public static let viewType = "xue_hua_video_player/view"

  public static func register(with registrar: FlutterPluginRegistrar) {
    registrar.register(
      XueHuaVideoViewFactory(messenger: registrar.messenger()),
      withId: viewType
    )
  }
}

final class XueHuaVideoViewFactory: NSObject, FlutterPlatformViewFactory {
  private let messenger: FlutterBinaryMessenger

  init(messenger: FlutterBinaryMessenger) {
    self.messenger = messenger
    super.init()
  }

  func create(
    withFrame frame: CGRect,
    viewIdentifier viewId: Int64,
    arguments args: Any?
  ) -> FlutterPlatformView {
    var playerId: Int64 = 0
    if let dict = args as? [String: Any], let id = dict["playerId"] as? NSNumber {
      playerId = id.int64Value
    }
    return XueHuaVideoPlatformView(frame: frame, playerId: playerId)
  }

  func createArgsCodec() -> FlutterMessageCodec & NSObjectProtocol {
    FlutterStandardMessageCodec.sharedInstance()
  }
}

final class XueHuaVideoPlatformView: UIView, FlutterPlatformView {
  private let playerId: Int64

  init(frame: CGRect, playerId: Int64) {
    self.playerId = playerId
    super.init(frame: frame)
    backgroundColor = .black
  }

  required init?(coder: NSCoder) {
    fatalError("init(coder:) has not been implemented")
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    guard bounds.width > 0, bounds.height > 0 else { return }
    XueHuaVideoPlayerBindings.onViewResized(playerId: playerId, view: self)
  }

  func view() -> UIView {
    self
  }

  deinit {
    XueHuaVideoPlayerBindings.onViewDestroyed(playerId: playerId)
  }
}

enum XueHuaVideoPlayerBindings {
  static func onViewResized(playerId: Int64, view: UIView) {
    let handle = UInt(bitPattern: Unmanaged.passUnretained(view).toOpaque())
    player_set_video_overlay_window(playerId, Int64(handle))
    let width = Int32(view.bounds.width)
    let height = Int32(view.bounds.height)
    if width > 0 && height > 0 {
      player_sync_video_overlay_rectangle(playerId, width, height)
    }
  }

  static func onViewDestroyed(playerId: Int64) {
    player_set_video_overlay_window(playerId, 0)
  }
}

@_silgen_name("player_set_video_overlay_window")
func player_set_video_overlay_window(_ playerId: Int64, _ windowHandle: Int64)

@_silgen_name("player_sync_video_overlay_rectangle")
func player_sync_video_overlay_rectangle(_ playerId: Int64, _ width: Int32, _ height: Int32)
