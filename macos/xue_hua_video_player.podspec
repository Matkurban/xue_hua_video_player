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
  s.source_files     = 'Classes/**/*'
  s.dependency 'FlutterMacOS'

  s.platform = :osx, '10.13'
  s.swift_version = '5.0'

  # --- GStreamer (macOS) -----------------------------------------------------
  # Official universal GStreamer.framework is auto-downloaded to the user cache
  # during pod install (ensure_gstreamer_macos.sh) and embedded into the .app
  # via vendored_frameworks + CocoaPods [CP] Embed Pods Frameworks.
  #
  # Set XUE_HUA_ALLOW_HOMEBREW_GSTREAMER=1 for local Homebrew-only dev (not MAS).
  gst_ver = ENV.fetch('GST_VER', '1.28.4')
  cache_root = File.expand_path("~/Library/Caches/xue_hua_video_player/gstreamer/#{gst_ver}")
  use_homebrew = ENV['XUE_HUA_ALLOW_HOMEBREW_GSTREAMER'] == '1'

  gst_sys_pkgs = %w[
    GLIB_2_0 GOBJECT_2_0 GIO_2_0
    GSTREAMER_1_0 GSTREAMER_BASE_1_0 GSTREAMER_APP_1_0 GSTREAMER_VIDEO_1_0
  ]

  if use_homebrew
    brew_prefix = `command -v brew >/dev/null 2>&1 && brew --prefix 2>/dev/null`.strip
    brew_prefix = '/opt/homebrew' if brew_prefix.empty?
    gst_pkg_config_path = ENV['GSTREAMER_PKG_CONFIG_PATH']
    gst_pkg_config_path = "#{brew_prefix}/lib/pkgconfig" if gst_pkg_config_path.nil? || gst_pkg_config_path.empty?
    gst_modules = 'gstreamer-1.0 gstreamer-app-1.0 gstreamer-video-1.0 gstreamer-base-1.0 gio-2.0 gobject-2.0 glib-2.0'
    gst_libs = `PKG_CONFIG_PATH="#{gst_pkg_config_path}" pkg-config --libs #{gst_modules} 2>/dev/null`.strip
    if gst_libs.empty?
      raise "Homebrew GStreamer not found (PKG_CONFIG_PATH=#{gst_pkg_config_path}). Install via brew or unset XUE_HUA_ALLOW_HOMEBREW_GSTREAMER"
    end
    rust_build_script = 'export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"; ' \
      "export PKG_CONFIG_PATH=\"#{gst_pkg_config_path}:$PKG_CONFIG_PATH\"; " \
      'sh "$PODS_TARGET_SRCROOT/../cargokit/build_pod.sh" ../rust xue_hua_video_player'
    other_ldflags = '-force_load ${PODS_CONFIGURATION_BUILD_DIR}/xue_hua_video_player/libxue_hua_video_player.a ' + gst_libs
    framework_search_paths = nil
    Pod::UI.puts '[xue_hua_video_player] Using Homebrew GStreamer (debug only; not suitable for Mac App Store)'
    s.user_target_xcconfig = {
      'XUE_HUA_ALLOW_HOMEBREW_GSTREAMER' => '1',
    }
  else
    ensure_script = File.join(__dir__, 'scripts', 'ensure_gstreamer_macos.sh')
    unless system({ 'GST_VER' => gst_ver }, 'sh', ensure_script)
      raise 'GStreamer ensure failed; check network connectivity or set XUE_HUA_GSTREAMER_ROOT / GSTREAMER_FRAMEWORK_SRC'
    end

    framework_path = "#{cache_root}/GStreamer.framework"
    if File.file?("#{framework_path}/Headers/gst/gst.h")
      framework_root = cache_root
    elsif File.file?('/Library/Frameworks/GStreamer.framework/Headers/gst/gst.h')
      framework_root = '/Library/Frameworks'
      framework_path = '/Library/Frameworks/GStreamer.framework'
    else
      raise <<~MSG
        GStreamer.framework not found after ensure step.
        Expected cache at #{cache_root} or system install at /Library/Frameworks.
        Set XUE_HUA_ALLOW_HOMEBREW_GSTREAMER=1 for Homebrew-only local dev.
      MSG
    end
    gst_headers = "#{framework_path}/Headers"

    Pod::UI.puts "[xue_hua_video_player] Using GStreamer.framework at #{framework_path}"

    link_script = File.join(__dir__, 'scripts', 'link_vendored_gstreamer.sh')
    unless system({ 'GST_VER' => gst_ver }, 'sh', link_script)
      raise 'GStreamer vendored link failed; see log above'
    end
    s.vendored_frameworks = 'Vendored/GStreamer.framework'

    gst_env = gst_sys_pkgs.map { |p|
      "export SYSTEM_DEPS_#{p}_NO_PKG_CONFIG=1; " \
      "export SYSTEM_DEPS_#{p}_SEARCH_FRAMEWORK=\"#{framework_root}\"; " \
      "export SYSTEM_DEPS_#{p}_LIB_FRAMEWORK=GStreamer; " \
      "export SYSTEM_DEPS_#{p}_INCLUDE=\"#{gst_headers}\"; "
    }.join
    rust_build_script = 'export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"; ' \
      'export PKG_CONFIG_ALLOW_CROSS=1; ' + gst_env +
      'sh "$PODS_TARGET_SRCROOT/../cargokit/build_pod.sh" ../rust xue_hua_video_player'
    other_ldflags = '-force_load ${PODS_CONFIGURATION_BUILD_DIR}/xue_hua_video_player/libxue_hua_video_player.a ' \
      '-framework GStreamer -liconv -lresolv -lz -lbz2 ' \
      '-framework CoreFoundation -framework CoreMedia -framework CoreVideo ' \
      '-framework CoreAudio -framework AVFoundation -framework AVFAudio ' \
      '-framework AudioToolbox -framework VideoToolbox -framework Foundation -framework Security'
    framework_search_paths = framework_root
  end

  s.script_phases = [
    {
      :name => 'Build Rust library',
      :script => rust_build_script,
      :execution_position => :before_compile,
      :input_files => ['${BUILT_PRODUCTS_DIR}/cargokit_phony'],
      :output_files => ['${PODS_CONFIGURATION_BUILD_DIR}/xue_hua_video_player/libxue_hua_video_player.a'],
    },
  ]

  pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'OTHER_LDFLAGS' => other_ldflags,
  }
  if use_homebrew
    pod_target_xcconfig['EXCLUDED_ARCHS[sdk=macosx*]'] = 'x86_64'
  end
  if framework_search_paths
    pod_target_xcconfig['FRAMEWORK_SEARCH_PATHS'] = framework_search_paths
  end
  s.pod_target_xcconfig = pod_target_xcconfig
end
