/// 内置 [VideoControls] 采用的视觉语言 / Selects which visual language [VideoControls] use.
enum VideoControlsStyle {
  /// iOS/macOS 用 Cupertino，其余用 Material / Cupertino on iOS/macOS, Material elsewhere.
  adaptive,

  /// 始终 Material 控件栏 / Always Material control bar.
  material,

  /// 始终 Cupertino 控件栏 / Always Cupertino control bar.
  cupertino,
}
