# Injects a Runner Run Script that embeds GStreamer.framework into the .app.
#
# Required when Flutter integrates this plugin via Swift Package Manager (SPM):
# Package.swift links -framework GStreamer, but CocoaPods vendored_frameworks
# is skipped, so nothing copies the framework into Contents/Frameworks/.
#
# In the host macos/Podfile:
#
#   require 'json'
#   plugins = JSON.parse(File.read(File.expand_path('../.flutter-plugins-dependencies', __dir__)))
#   xhvp = plugins.dig('plugins', 'macos')&.find { |p| p['name'] == 'xue_hua_video_player' }
#   raise 'xue_hua_video_player not found in .flutter-plugins-dependencies' unless xhvp
#   require File.expand_path('macos/gstreamer_podfile_helper.rb', xhvp['path'])
#
#   post_install do |installer|
#     # ... existing flutter_additional_macos_build_settings ...
#     install_gstreamer_embed_script!(installer)
#   end
#
# Pure-SPM hosts without a Podfile: add the same Run Script manually (see example).

PHASE_NAME = '[xue_hua_video_player] Embed GStreamer Framework'

def remove_gstreamer_embed_script!(installer)
  podfile_dir = Pod::Config.instance.installation_root.to_s
  runner_project_path = File.join(podfile_dir, 'Runner.xcodeproj')
  return unless File.directory?(runner_project_path)

  require 'xcodeproj'
  project = Xcodeproj::Project.open(runner_project_path)
  target = project.targets.find { |t| t.name == 'Runner' }
  return unless target

  removed = target.build_phases
    .grep(Xcodeproj::Project::Object::PBXShellScriptBuildPhase)
    .select { |p| p.name == PHASE_NAME }
  return if removed.empty?

  removed.each(&:remove_from_project)
  project.save
  Pod::UI.puts '[xue_hua_video_player] Removed GStreamer embed Run Script from Runner'
end

def install_gstreamer_embed_script!(installer)
  podfile_dir = Pod::Config.instance.installation_root.to_s
  runner_project_path = File.join(podfile_dir, 'Runner.xcodeproj')
  unless File.directory?(runner_project_path)
    Pod::UI.puts "[xue_hua_video_player] #{runner_project_path} not found; skipping GStreamer embed"
    return
  end

  require 'xcodeproj'
  project = Xcodeproj::Project.open(runner_project_path)
  target = project.targets.find { |t| t.name == 'Runner' }
  unless target
    Pod::UI.puts '[xue_hua_video_player] Runner target not found; skipping GStreamer embed'
    return
  end

  plugin_macos_dir = File.expand_path(__dir__)
  paths_script = File.join(plugin_macos_dir, 'scripts', 'gstreamer_paths.sh')
  embed_script = File.join(plugin_macos_dir, 'scripts', 'embed_gstreamer_framework.sh')
  unless File.file?(embed_script)
    Pod::UI.puts "[xue_hua_video_player] embed script missing at #{embed_script}"
    return
  end

  gst_ver = ENV.fetch('GST_VER', '1.28.4')
  default_cache_sdk = File.expand_path(
    "~/Library/Caches/xue_hua_video_player/gstreamer/#{gst_ver}/GStreamer.framework/Versions/Current",
  )
  default_cache_runtime = File.expand_path(
    "~/Library/Caches/xue_hua_video_player/gstreamer/#{gst_ver}/GStreamerRuntime.framework/Versions/Current",
  )

  target.build_phases
    .grep(Xcodeproj::Project::Object::PBXShellScriptBuildPhase)
    .select { |p| p.name == PHASE_NAME }
    .each(&:remove_from_project)

  phase = target.new_shell_script_build_phase(PHASE_NAME)
  phase.shell_script = <<~SCRIPT
    set -euo pipefail
    export XUE_HUA_ALLOW_HOMEBREW_GSTREAMER="${XUE_HUA_ALLOW_HOMEBREW_GSTREAMER:-}"
    # shellcheck source=gstreamer_paths.sh
    source "#{paths_script}"
    bash "#{embed_script}"
  SCRIPT
  phase.input_paths = [
    default_cache_runtime,
    default_cache_sdk,
    '/Library/Frameworks/GStreamer.framework/Versions/Current',
  ]
  phase.output_paths = ['${TARGET_BUILD_DIR}/${FRAMEWORKS_FOLDER_PATH}/GStreamer.framework']
  phase.always_out_of_date = '1'

  project.save
  Pod::UI.puts '[xue_hua_video_player] Added GStreamer embed Run Script to Runner (required under SPM)'
end
