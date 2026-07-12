#
# To learn more about a Podspec see http://guides.cocoapods.org/syntax/podspec.html.
#
Pod::Spec.new do |s|
  s.name             = 'xue_hua_video_player'
  s.version          = '1.0.0'
  s.summary          = 'GStreamer-backed video player Flutter plugin.'
  s.description      = <<-DESC
A Flutter video player plugin that decodes local/network video with GStreamer
(via a native C core + Dart FFI) and renders into Flutter Texture widgets.
                       DESC
  s.homepage         = 'https://github.com/Matkurban/xue_hua_video_player'
  s.license          = { :file => '../LICENSE' }
  s.author           = { 'Matkurban' => '3496354336@qq.com' }
  s.module_name      = 'xue_hua_video_player'

  s.source           = { :path => '.' }
  s.source_files = 'xue_hua_video_player/Sources/xue_hua_video_player/**/*.{swift,h,m,c}'
  s.dependency 'Flutter'
  s.platform = :ios, '13.0'
  s.swift_version = '5.0'

  gst_root = ENV['GSTREAMER_ROOT_IOS']
  if gst_root.nil? || gst_root.empty?
    gst_root = "#{ENV['HOME']}/Library/Developer/GStreamer/iPhone.sdk"
  end
  gst_framework_parent = gst_root
  gst_headers = "#{gst_root}/GStreamer.framework/Headers"

  s.script_phase = {
    :name => 'Build C player library',
    :script => 'export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"; export XHVP_IOS=1; export GSTREAMER_FRAMEWORK_ROOT="' + gst_root + '"; export PLATFORM_NAME="${PLATFORM_NAME}"; sh "$PODS_TARGET_SRCROOT/../native/scripts/build_pod.sh"',
    :execution_position => :before_compile,
    :input_files => [
      '${PODS_TARGET_SRCROOT}/../native/include/xhvp_player.h',
      '${PODS_TARGET_SRCROOT}/../native/src/pipeline.c',
      '${PODS_TARGET_SRCROOT}/../native/src/thumbnail.c',
      '${PODS_TARGET_SRCROOT}/../native/src/bus.c',
      '${PODS_TARGET_SRCROOT}/../native/src/xhvp_player.c',
      '${PODS_TARGET_SRCROOT}/../native/src/xhvp_ffi_keep.c',
    ],
    :output_files => ["${PODS_CONFIGURATION_BUILD_DIR}/xue_hua_video_player/libxue_hua_video_player.a"],
  }

  force_load = '-force_load ${PODS_CONFIGURATION_BUILD_DIR}/xue_hua_video_player/libxue_hua_video_player.a'

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386',
    'ENABLE_BITCODE' => 'NO',
    'HEADER_SEARCH_PATHS' => '"' + gst_headers + '"',
    'FRAMEWORK_SEARCH_PATHS' => '"' + gst_framework_parent + '"',
    'OTHER_LDFLAGS' => force_load + ' -framework GStreamer -liconv -lresolv -lz -lbz2 -framework UIKit -framework QuartzCore -framework CoreGraphics -framework IOSurface -framework Metal -framework CoreFoundation -framework CoreMedia -framework CoreVideo -framework CoreAudio -framework AVFoundation -framework AVFAudio -framework AssetsLibrary -framework AudioToolbox -framework VideoToolbox -framework OpenGLES -framework Foundation -framework Security',
  }
  # Runner link + keep global symbols for Dart DynamicLibrary.process() / dlsym.
  s.user_target_xcconfig = {
    'OTHER_LDFLAGS' => force_load,
    'STRIP_STYLE' => 'non-global',
  }
  s.vendored_frameworks = []
end
