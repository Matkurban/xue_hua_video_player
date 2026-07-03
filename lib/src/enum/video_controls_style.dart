/// Selects which visual language the built-in [VideoControls] use.
enum VideoControlsStyle {
  /// Pick Cupertino on iOS/macOS, Material elsewhere.
  adaptive,

  /// Always use the Material control bar.
  material,

  /// Always use the Cupertino control bar.
  cupertino,
}
