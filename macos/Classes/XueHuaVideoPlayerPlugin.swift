import Cocoa
import FlutterMacOS

public class XueHuaVideoPlayerPlugin: NSObject, FlutterPlugin {
  public static let viewType = "xue_hua_video_player/view"

  public static func register(with registrar: FlutterPluginRegistrar) {
    registrar.register(
      XueHuaVideoViewFactory(messenger: registrar.messenger),
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
    withViewIdentifier viewId: Int64,
    arguments args: Any?
  ) -> NSView {
    var playerId: Int64 = 0
    if let dict = args as? [String: Any], let id = dict["playerId"] as? NSNumber {
      playerId = id.int64Value
    }
    let view = XueHuaVideoPlatformView(frame: .zero, playerId: playerId)
    return view
  }

  func createArgsCodec() -> (FlutterMessageCodec & NSObjectProtocol)? {
    FlutterStandardMessageCodec.sharedInstance()
  }
}

final class XueHuaVideoPlatformView: NSView {
  private let playerId: Int64
  private let videoHostView = NSView()
  private var lastBoundViewPtr: Int64 = 0
  private var lastSyncedWidth: Int32 = 0
  private var lastSyncedHeight: Int32 = 0
  private var pendingApplyWidth: Int32 = 0
  private var pendingApplyHeight: Int32 = 0
  private var applyScheduled = false

  init(frame: NSRect, playerId: Int64) {
    self.playerId = playerId
    super.init(frame: frame)
    wantsLayer = true

    videoHostView.wantsLayer = false
    videoHostView.autoresizingMask = [.width, .height]
    addSubview(videoHostView)
  }

  required init?(coder: NSCoder) {
    fatalError("init(coder:) has not been implemented")
  }

  override func layout() {
    super.layout()
    videoHostView.frame = bounds
    syncOverlayIfNeeded(bindHandle: false)
  }

  override func viewDidMoveToWindow() {
    super.viewDidMoveToWindow()
    if window != nil {
      syncOverlayIfNeeded(bindHandle: true)
    }
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
      NSLog(
        "xue_hua_video_player: bind overlay playerId=%lld hostView=%lld",
        playerId, viewPtr
      )
      player_set_video_overlay_window(playerId, viewPtr)
      scheduleOverlayApply(width: 0, height: 0)
      return
    }

    guard width > 0, height > 0 else { return }

    if width == lastSyncedWidth && height == lastSyncedHeight && viewPtr == lastBoundViewPtr {
      return
    }
    lastSyncedWidth = width
    lastSyncedHeight = height
    lastBoundViewPtr = viewPtr

    NSLog(
      "xue_hua_video_player: sync overlay size playerId=%lld hostView=%lld %dx%d",
      playerId, viewPtr, width, height
    )
    player_sync_macos_video_layer(playerId, viewPtr, width, height)
    scheduleOverlayApply(width: width, height: height)
  }

  /// Applies the cached overlay handle on the main thread after layout returns.
  ///
  /// `osxvideosink` calls `setView:` directly when invoked on the main thread;
  /// calling from a background thread blocks with `performSelector:waitUntilDone:YES`.
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
      player_apply_macos_overlay_gstreamer(self.playerId, w, h)
    }
  }

  deinit {
    let id = playerId
    player_set_video_overlay_window(id, 0)
    DispatchQueue.main.async {
      player_apply_macos_overlay_gstreamer(id, 0, 0)
    }
  }
}

@_silgen_name("player_set_video_overlay_window")
func player_set_video_overlay_window(_ playerId: Int64, _ windowHandle: Int64)

@_silgen_name("player_sync_macos_video_layer")
func player_sync_macos_video_layer(
  _ playerId: Int64, _ nsViewPtr: Int64, _ width: Int32, _ height: Int32
)

@_silgen_name("player_apply_macos_overlay_gstreamer")
func player_apply_macos_overlay_gstreamer(
  _ playerId: Int64, _ width: Int32, _ height: Int32
)
