/// 跨平台 GStreamer 视频播放器 Flutter 插件公开 API / Public API for the cross-platform GStreamer video player plugin.
///
/// 本库导出播放器初始化、控制器、视图、媒体源描述符及内置控件主题。
/// Exports player initialization, controllers, views, media source descriptors, and built-in control theming.
///
/// # 主要导出 / Main exports
/// - [XueHuaVideoPlayer] — 一次性初始化 Rust 桥 / one-time Rust bridge init
/// - [XueHuaPlayerController] — 播放控制与 reactive 状态 / playback control and reactive state
/// - [XueHuaVideoView] — 带可选控件栏的视频视图 / video view with optional control bar
/// - [VideoSource] — 网络/文件/资源媒体描述 / network, file, or asset media descriptor
/// - [VideoControls]、[VideoControlsTheme] — 内置控件与主题 / built-in controls and theming
library;

export 'src/controls/fullscreen_config.dart'
    show AspectRatioModeLabels, VideoControlsFullscreenConfig;
export 'src/controls/playback_controls_model.dart' show PlaybackControlsModel;
export 'src/presentation/playback_presentation_model.dart'
    show PlaybackPresentationModel;
export 'src/controls/video_controls.dart' show VideoControls;
export 'src/enum/video_controls_style.dart';
export 'src/xue_hua_player_controller.dart';
export 'src/theme/video_controls_theme.dart';
export 'src/model/video_source.dart';
export 'src/xue_hua_video_view.dart';
export 'src/xue_hua_video_player.dart' show XueHuaVideoPlayer;
