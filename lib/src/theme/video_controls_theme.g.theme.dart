// dart format width=80
// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint, unused_element

part of 'video_controls_theme.dart';

// **************************************************************************
// ThemeExtensionsGenerator
// **************************************************************************

mixin _$VideoControlsTheme on ThemeExtension<VideoControlsTheme> {
  @override
  ThemeExtension<VideoControlsTheme> copyWith({
    Color? iconColor,
    Color? activeIconColor,
    Color? activeTrackColor,
    Color? inactiveTrackColor,
    Color? bufferedTrackColor,
    Color? thumbColor,
    Color? textColor,
    Color? backgroundColor,
    double? primaryIconSize,
    double? secondaryIconSize,
    EdgeInsets? barPadding,
    double? borderRadius,
  }) {
    final _this = (this as VideoControlsTheme);

    return VideoControlsTheme(
      iconColor: iconColor ?? _this.iconColor,
      activeIconColor: activeIconColor ?? _this.activeIconColor,
      activeTrackColor: activeTrackColor ?? _this.activeTrackColor,
      inactiveTrackColor: inactiveTrackColor ?? _this.inactiveTrackColor,
      bufferedTrackColor: bufferedTrackColor ?? _this.bufferedTrackColor,
      thumbColor: thumbColor ?? _this.thumbColor,
      textColor: textColor ?? _this.textColor,
      backgroundColor: backgroundColor ?? _this.backgroundColor,
      primaryIconSize: primaryIconSize ?? _this.primaryIconSize,
      secondaryIconSize: secondaryIconSize ?? _this.secondaryIconSize,
      barPadding: barPadding ?? _this.barPadding,
      borderRadius: borderRadius ?? _this.borderRadius,
    );
  }

  @override
  ThemeExtension<VideoControlsTheme> lerp(
    ThemeExtension<VideoControlsTheme>? other,
    double t,
  ) {
    if (other is! VideoControlsTheme) {
      return this;
    }

    final _this = (this as VideoControlsTheme);

    return VideoControlsTheme(
      iconColor: Color.lerp(_this.iconColor, other.iconColor, t)!,
      activeIconColor: Color.lerp(
        _this.activeIconColor,
        other.activeIconColor,
        t,
      )!,
      activeTrackColor: Color.lerp(
        _this.activeTrackColor,
        other.activeTrackColor,
        t,
      )!,
      inactiveTrackColor: Color.lerp(
        _this.inactiveTrackColor,
        other.inactiveTrackColor,
        t,
      )!,
      bufferedTrackColor: Color.lerp(
        _this.bufferedTrackColor,
        other.bufferedTrackColor,
        t,
      )!,
      thumbColor: Color.lerp(_this.thumbColor, other.thumbColor, t)!,
      textColor: Color.lerp(_this.textColor, other.textColor, t)!,
      backgroundColor: Color.lerp(
        _this.backgroundColor,
        other.backgroundColor,
        t,
      )!,
      primaryIconSize: lerpDouble$(
        _this.primaryIconSize,
        other.primaryIconSize,
        t,
      )!,
      secondaryIconSize: lerpDouble$(
        _this.secondaryIconSize,
        other.secondaryIconSize,
        t,
      )!,
      barPadding: EdgeInsets.lerp(_this.barPadding, other.barPadding, t)!,
      borderRadius: lerpDouble$(_this.borderRadius, other.borderRadius, t)!,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) {
      return true;
    }

    if (other.runtimeType != runtimeType) {
      return false;
    }

    final _this = (this as VideoControlsTheme);
    final _other = (other as VideoControlsTheme);

    return _other.iconColor == _this.iconColor &&
        _other.activeIconColor == _this.activeIconColor &&
        _other.activeTrackColor == _this.activeTrackColor &&
        _other.inactiveTrackColor == _this.inactiveTrackColor &&
        _other.bufferedTrackColor == _this.bufferedTrackColor &&
        _other.thumbColor == _this.thumbColor &&
        _other.textColor == _this.textColor &&
        _other.backgroundColor == _this.backgroundColor &&
        _other.primaryIconSize == _this.primaryIconSize &&
        _other.secondaryIconSize == _this.secondaryIconSize &&
        _other.barPadding == _this.barPadding &&
        _other.borderRadius == _this.borderRadius;
  }

  @override
  int get hashCode {
    final _this = (this as VideoControlsTheme);

    return Object.hash(
      runtimeType,
      _this.iconColor,
      _this.activeIconColor,
      _this.activeTrackColor,
      _this.inactiveTrackColor,
      _this.bufferedTrackColor,
      _this.thumbColor,
      _this.textColor,
      _this.backgroundColor,
      _this.primaryIconSize,
      _this.secondaryIconSize,
      _this.barPadding,
      _this.borderRadius,
    );
  }
}

extension VideoControlsThemeBuildContext on BuildContext {
  VideoControlsTheme get videoControlsTheme =>
      Theme.of(this).extension<VideoControlsTheme>()!;
}
