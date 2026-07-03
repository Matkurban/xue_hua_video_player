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
(via a Rust flutter_rust_bridge core) and renders into a Flutter texture.
                       DESC
  s.homepage         = 'https://github.com/Matkurban/xue_hua_video_player'
  s.license          = { :file => '../LICENSE' }
  s.author           = { 'Matkurban' => '3496354336@qq.com' }
  s.module_name      = 'xue_hua_video_player'

  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'FlutterMacOS'

  s.platform = :osx, '10.13'
  s.swift_version = '5.0'

  # --- GStreamer discovery ---------------------------------------------------
  # Locate GStreamer's pkg-config metadata. Override GSTREAMER_PKG_CONFIG_PATH to
  # point at a custom install (e.g. the official GStreamer.framework:
  #   /Library/Frameworks/GStreamer.framework/Versions/1.0/lib/pkgconfig ).
  # Defaults to a Homebrew install (`brew install gstreamer gst-plugins-{base,good,bad}`).
  gst_pkg_config_path = ENV['GSTREAMER_PKG_CONFIG_PATH']
  if gst_pkg_config_path.nil? || gst_pkg_config_path.empty?
    brew_prefix = `command -v brew >/dev/null 2>&1 && brew --prefix 2>/dev/null`.strip
    brew_prefix = '/opt/homebrew' if brew_prefix.empty?
    gst_pkg_config_path = "#{brew_prefix}/lib/pkgconfig"
  end

  gst_modules = 'gstreamer-1.0 gstreamer-app-1.0 gstreamer-video-1.0 gstreamer-base-1.0 gio-2.0 gobject-2.0 glib-2.0'
  gst_libs = `PKG_CONFIG_PATH="#{gst_pkg_config_path}" pkg-config --libs #{gst_modules} 2>/dev/null`.strip
  raise "Unable to locate GStreamer via pkg-config (PKG_CONFIG_PATH=#{gst_pkg_config_path}). Install GStreamer or set GSTREAMER_PKG_CONFIG_PATH." if gst_libs.empty?

  s.script_phase = {
    :name => 'Build Rust library',
    # Export the pkg-config location so gstreamer-sys can find GStreamer while
    # cargokit compiles the Rust static library.
    :script => 'export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"; export PKG_CONFIG_PATH="' + gst_pkg_config_path + ':$PKG_CONFIG_PATH"; sh "$PODS_TARGET_SRCROOT/../cargokit/build_pod.sh" ../rust xue_hua_video_player',
    :execution_position => :before_compile,
    :input_files => ['${BUILT_PRODUCTS_DIR}/cargokit_phony'],
    :output_files => ["${PODS_CONFIGURATION_BUILD_DIR}/xue_hua_video_player/libxue_hua_video_player.a"],
  }

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386',
    # Force-load the Rust static library and link the GStreamer dylibs it needs.
    'OTHER_LDFLAGS' => '-force_load ${PODS_CONFIGURATION_BUILD_DIR}/xue_hua_video_player/libxue_hua_video_player.a ' + gst_libs,
  }
end
