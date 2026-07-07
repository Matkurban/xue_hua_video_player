import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';

import 'video_surface_handle.dart';

/// Platform view type registered by native plugin code.
const String kXueHuaVideoViewType = 'xue_hua_video_player/view';

/// Builds an embedded PlatformView for Android, iOS, or macOS.
Widget buildMobilePlatformView(VideoSurfaceHandle handle) {
  assert(handle.kind == VideoSurfaceKind.platformView);
  final creationParams = <String, dynamic>{'playerId': handle.playerId};
  const paramsCodec = StandardMessageCodec();

  switch (defaultTargetPlatform) {
    case TargetPlatform.android:
      return Builder(
        builder: (context) {
          final layoutDirection =
              Directionality.maybeOf(context) ?? TextDirection.ltr;
          return PlatformViewLink(
            viewType: kXueHuaVideoViewType,
            surfaceFactory:
                (BuildContext context, PlatformViewController controller) {
                  return AndroidViewSurface(
                    controller: controller as AndroidViewController,
                    gestureRecognizers:
                        const <Factory<OneSequenceGestureRecognizer>>{},
                    hitTestBehavior: PlatformViewHitTestBehavior.opaque,
                  );
                },
            onCreatePlatformView: (PlatformViewCreationParams params) {
              return PlatformViewsService.initSurfaceAndroidView(
                  id: params.id,
                  viewType: kXueHuaVideoViewType,
                  layoutDirection: layoutDirection,
                  creationParams: creationParams,
                  creationParamsCodec: paramsCodec,
                  onFocus: () => params.onFocusChanged(true),
                )
                ..addOnPlatformViewCreatedListener(params.onPlatformViewCreated)
                ..create();
            },
          );
        },
      );
    case TargetPlatform.iOS:
      return UiKitView(
        viewType: kXueHuaVideoViewType,
        creationParams: creationParams,
        creationParamsCodec: paramsCodec,
        gestureRecognizers: const <Factory<OneSequenceGestureRecognizer>>{},
      );
    case TargetPlatform.macOS:
      return AppKitView(
        viewType: kXueHuaVideoViewType,
        creationParams: creationParams,
        creationParamsCodec: paramsCodec,
        gestureRecognizers: const <Factory<OneSequenceGestureRecognizer>>{},
      );
    default:
      return ColoredBox(
        color: Colors.black,
        child: Center(
          child: Text('PlatformView not supported on $defaultTargetPlatform'),
        ),
      );
  }
}
