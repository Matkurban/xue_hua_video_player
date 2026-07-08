/// 顺时针视频旋转角度 / Clockwise video rotation in degrees.
enum VideoRotation {
  /// 不旋转 / No rotation.
  deg0(0),

  /// 顺时针 90° / Clockwise 90 degrees.
  deg90(90),

  /// 顺时针 180° / Clockwise 180 degrees.
  deg180(180),

  /// 顺时针 270° / Clockwise 270 degrees.
  deg270(270);

  /// 创建旋转枚举 / Creates a rotation value.
  const VideoRotation(this.degrees);

  /// 顺时针角度：0、90、180 或 270 / Clockwise degrees: 0, 90, 180, or 270.
  final int degrees;

  /// 由角度解析枚举；未知值回退为 [deg0] / Parses enum from degrees; unknown values map to [deg0].
  static VideoRotation fromDegrees(int degrees) => switch (degrees) {
    90 => VideoRotation.deg90,
    180 => VideoRotation.deg180,
    270 => VideoRotation.deg270,
    _ => VideoRotation.deg0,
  };
}
