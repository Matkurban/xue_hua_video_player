#
# To learn more about a Podspec see http://guides.cocoapods.org/syntax/podspec.html.
# Run `pod lib lint xue_hua_video_player.podspec` to validate before publishing.
#
Pod::Spec.new do |s|
  s.name             = 'xue_hua_video_player'
  s.version          = '1.0.0'
  s.summary          = 'GStreamer-backed video player Flutter plugin.'
  s.description      = <<-DESC
A Flutter video player plugin that decodes local/network video with GStreamer
(via a Rust flutter_rust_bridge core) and renders into Flutter Platform Views.
                       DESC
  s.homepage         = 'https://github.com/Matkurban/xue_hua_video_player'
  s.license          = { :file => '../LICENSE' }
  s.author           = { 'Matkurban' => '3496354336@qq.com' }
  s.module_name      = 'xue_hua_video_player'

  s.source           = { :path => '.' }
  s.source_files = 'Classes/**/*.swift'
  s.dependency 'Flutter'
  s.platform = :ios, '13.0'
  s.swift_version = '5.0'

  # --- GStreamer (iOS) discovery ---------------------------------------------
  # iOS requires the official GStreamer iOS SDK (a static "GStreamer.framework").
  # Download it from https://gstreamer.freedesktop.org/download/ and install; it
  # lands at ~/Library/Developer/GStreamer/iPhone.sdk/GStreamer.framework .
  # Override GSTREAMER_ROOT_IOS to point elsewhere.
  gst_root = ENV['GSTREAMER_ROOT_IOS']
  if gst_root.nil? || gst_root.empty?
    gst_root = "#{ENV['HOME']}/Library/Developer/GStreamer/iPhone.sdk"
  end
  gst_framework_parent = gst_root
  gst_headers = "#{gst_root}/GStreamer.framework/Headers"

  # The GStreamer iOS SDK ships a single *static* "GStreamer.framework" with
  # flattened headers and NO pkg-config (.pc) files. Rather than pkg-config, we
  # drive the Rust gstreamer/glib `-sys` crates through `system-deps` env
  # overrides (NO_PKG_CONFIG), pointing them at the umbrella framework. Cargo
  # builds a staticlib, so the actual link of "-framework GStreamer" happens in
  # Xcode via OTHER_LDFLAGS below.
  gst_sys_pkgs = %w[
    GLIB_2_0 GOBJECT_2_0 GIO_2_0
    GSTREAMER_1_0 GSTREAMER_BASE_1_0 GSTREAMER_APP_1_0 GSTREAMER_VIDEO_1_0
  ]
  gst_env = gst_sys_pkgs.map { |p|
    "export SYSTEM_DEPS_#{p}_NO_PKG_CONFIG=1; " \
    "export SYSTEM_DEPS_#{p}_SEARCH_FRAMEWORK=\"#{gst_framework_parent}\"; " \
    "export SYSTEM_DEPS_#{p}_LIB_FRAMEWORK=GStreamer; " \
    "export SYSTEM_DEPS_#{p}_INCLUDE=\"#{gst_headers}\"; "
  }.join

  s.script_phase = {
    :name => 'Build Rust library',
    :script => 'export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"; export GSTREAMER_ROOT_IOS="' + gst_root + '"; export PKG_CONFIG_ALLOW_CROSS=1; ' + gst_env + '. "$PODS_TARGET_SRCROOT/scripts/ios_rust_link_flags.sh"; sh "$PODS_TARGET_SRCROOT/../cargokit/build_pod.sh" ../rust xue_hua_video_player',
    :execution_position => :before_compile,
    :input_files => ['${BUILT_PRODUCTS_DIR}/cargokit_phony'],
    :output_files => ["${PODS_CONFIGURATION_BUILD_DIR}/xue_hua_video_player/libxue_hua_video_player.a"],
  }

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386',
    'ENABLE_BITCODE' => 'NO',
    'FRAMEWORK_SEARCH_PATHS' => '"' + gst_framework_parent + '"',
    # Force-load the Rust static library and link the umbrella GStreamer
    # framework. iOS plugins are static and are registered explicitly by the
    # Rust core (`register_ios_static_plugins()` in `rust/src/player.rs`); the
    # linker pulls the referenced `gst_plugin_*_register` objects from the
    # framework archive. gstgl/glimagesink also needs UIKit/QuartzCore/OpenGLES.
    'OTHER_LDFLAGS' => '-force_load ${PODS_CONFIGURATION_BUILD_DIR}/xue_hua_video_player/libxue_hua_video_player.a -framework GStreamer -liconv -lresolv -lz -lbz2 -framework UIKit -framework QuartzCore -framework CoreGraphics -framework IOSurface -framework Metal -framework CoreFoundation -framework CoreMedia -framework CoreVideo -framework CoreAudio -framework AVFoundation -framework AVFAudio -framework AssetsLibrary -framework AudioToolbox -framework VideoToolbox -framework OpenGLES -framework Foundation -framework Security',
  }
  s.vendored_frameworks = []
end
