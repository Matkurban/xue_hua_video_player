import Flutter
import QuartzCore
import UIKit

public class XueHuaVideoPlayerPlugin: NSObject, FlutterPlugin {
  public static let viewType = "xue_hua_video_player/view"

  public static func register(with registrar: FlutterPluginRegistrar) {
    if let assetsDir = flutterAssetsDirectory() {
      xhvp_set_flutter_assets_dir(assetsDir)
    }
    registrar.register(
      XueHuaVideoViewFactory(messenger: registrar.messenger()),
      withId: viewType
    )
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
  private let videoHostView = UIView()
  private var lastBoundViewPtr: Int64 = 0
  private var lastSyncedWidth: Int32 = 0
  private var lastSyncedHeight: Int32 = 0
  private var pendingApplyWidth: Int32 = 0
  private var pendingApplyHeight: Int32 = 0
  private var applyScheduled = false

  init(frame: CGRect, playerId: Int64) {
    self.playerId = playerId
    super.init(frame: frame)
    backgroundColor = .black

    videoHostView.backgroundColor = .black
    videoHostView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    addSubview(videoHostView)
  }

  required init?(coder: NSCoder) {
    fatalError("init(coder:) has not been implemented")
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    videoHostView.frame = bounds
    syncOverlayIfNeeded(bindHandle: false)
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    if window != nil {
      configureHostLayerIfNeeded()
      syncOverlayIfNeeded(bindHandle: true)
    }
  }

  func view() -> UIView {
    self
  }

  private func configureHostLayerIfNeeded() {
    guard videoHostView.layer.sublayers == nil || videoHostView.layer.sublayers?.isEmpty == true
    else {
      return
    }
    videoHostView.layer.isOpaque = true
    videoHostView.layer.contentsScale = window?.screen.scale ?? UIScreen.main.scale
  }

  private func overlayViewPtr() -> Int64 {
    Int64(UInt(bitPattern: Unmanaged.passUnretained(videoHostView).toOpaque()))
  }

  private func syncOverlayIfNeeded(bindHandle: Bool) {
    let viewPtr = overlayViewPtr()
    let width = Int32(videoHostView.bounds.width)
    let height = Int32(videoHostView.bounds.height)

    if bindHandle && viewPtr != lastBoundViewPtr {
      lastBoundViewPtr = viewPtr
      lastSyncedWidth = 0
      lastSyncedHeight = 0
      player_notify_ios_overlay(playerId, viewPtr, width, height)
      scheduleOverlayApply(width: width, height: height)
      return
    }

    guard width > 0, height > 0 else { return }

    if width == lastSyncedWidth && height == lastSyncedHeight && viewPtr == lastBoundViewPtr {
      return
    }
    lastSyncedWidth = width
    lastSyncedHeight = height
    lastBoundViewPtr = viewPtr

    player_notify_ios_overlay(playerId, viewPtr, width, height)
    scheduleOverlayApply(width: width, height: height)
  }

  /// Attaches the GStreamer sink CALayer on the main thread after layout returns.
  private func scheduleOverlayApply(width: Int32, height: Int32) {
    pendingApplyWidth = width
    pendingApplyHeight = height
    guard !applyScheduled else { return }
    applyScheduled = true
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      self.applyScheduled = false
      let w = self.pendingApplyWidth
      let h = self.pendingApplyHeight
      player_apply_ios_overlay_gstreamer(self.playerId, w, h)
    }
  }

  deinit {
    let id = playerId
    player_notify_ios_overlay(id, 0, 0, 0)
    DispatchQueue.main.async {
      player_apply_ios_overlay_gstreamer(id, 0, 0)
    }
  }
}

@_silgen_name("xhvp_set_flutter_assets_dir")
func xhvp_set_flutter_assets_dir(_ path: UnsafePointer<CChar>)

@_silgen_name("player_notify_ios_overlay")
func player_notify_ios_overlay(_ playerId: Int64, _ windowHandle: Int64, _ width: Int32, _ height: Int32)

@_silgen_name("player_apply_ios_overlay_gstreamer")
func player_apply_ios_overlay_gstreamer(_ playerId: Int64, _ width: Int32, _ height: Int32)
