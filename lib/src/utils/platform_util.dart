import 'package:flutter/foundation.dart';

/// 是否为 iOS / Android 移动端平台 / Whether the current platform is mobile (iOS or Android).
bool get isMobilePlatform =>
    switch (defaultTargetPlatform) {
      TargetPlatform.iOS || TargetPlatform.android => true,
      _ => false,
    };
