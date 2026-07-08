//! iOS 静态链接 GStreamer 插件的显式注册。
//!
//! Explicit registration of statically linked GStreamer plugins on iOS.
//!
//! iOS 上 GStreamer 以单个静态 `GStreamer.framework` 形式交付。与 Android 类似，
//! 静态链接的插件无法通过文件系统扫描发现，因此必须逐个调用注册函数。

#[cfg(target_os = "ios")]
extern "C" {
    fn gst_plugin_coreelements_register();
    fn gst_plugin_app_register();
    fn gst_plugin_typefindfunctions_register();
    fn gst_plugin_playback_register();
    fn gst_plugin_autodetect_register();
    fn gst_plugin_pbtypes_register();
    fn gst_plugin_gio_register();
    fn gst_plugin_videoconvertscale_register();
    fn gst_plugin_videofilter_register();
    fn gst_plugin_videorate_register();
    fn gst_plugin_deinterlace_register();
    fn gst_plugin_videocrop_register();
    fn gst_plugin_audioconvert_register();
    fn gst_plugin_audioresample_register();
    fn gst_plugin_audiorate_register();
    fn gst_plugin_volume_register();
    fn gst_plugin_audiofx_register();
    fn gst_plugin_audioparsers_register();
    fn gst_plugin_videoparsersbad_register();
    fn gst_plugin_isomp4_register();
    fn gst_plugin_matroska_register();
    fn gst_plugin_id3demux_register();
    fn gst_plugin_subparse_register();
    fn gst_plugin_libav_register();
    fn gst_plugin_jpeg_register();
    fn gst_plugin_png_register();
    fn gst_plugin_osxaudio_register();
    fn gst_plugin_soup_register();
    fn gst_plugin_hls_register();
    fn gst_plugin_rtp_register();
    fn gst_plugin_rtpmanager_register();
    fn gst_plugin_rtsp_register();
    fn gst_plugin_udp_register();
    fn gst_plugin_tcp_register();
    fn gst_plugin_srtp_register();
    fn gst_plugin_dtls_register();
    fn gst_plugin_opengl_register();
    fn gst_plugin_applemedia_register();
}

/// 注册 iOS 框架中静态链接的全部 GStreamer 插件。
/// Registers the statically-linked GStreamer plugins bundled in the iOS framework.
///
/// # 参数 / Parameters
/// 无 / None.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 线程 / Threading
/// - 必须在 `xhvp-gst` 线程上调用（由 [`super::init::ensure_gst_init`] 保证）。
///   Must be called on the `xhvp-gst` thread (ensured by [`super::init::ensure_gst_init`]).
///
/// # 平台 / Platform
/// - 仅 **iOS** / **iOS** only.
#[cfg(target_os = "ios")]
pub fn register_ios_static_plugins() {
    unsafe {
        gst_plugin_coreelements_register();
        gst_plugin_app_register();
        gst_plugin_typefindfunctions_register();
        gst_plugin_playback_register();
        gst_plugin_autodetect_register();
        gst_plugin_pbtypes_register();
        gst_plugin_gio_register();
        gst_plugin_videoconvertscale_register();
        gst_plugin_videofilter_register();
        gst_plugin_videorate_register();
        gst_plugin_deinterlace_register();
        gst_plugin_videocrop_register();
        gst_plugin_audioconvert_register();
        gst_plugin_audioresample_register();
        gst_plugin_audiorate_register();
        gst_plugin_volume_register();
        gst_plugin_audiofx_register();
        gst_plugin_audioparsers_register();
        gst_plugin_videoparsersbad_register();
        gst_plugin_isomp4_register();
        gst_plugin_matroska_register();
        gst_plugin_id3demux_register();
        gst_plugin_subparse_register();
        gst_plugin_libav_register();
        gst_plugin_jpeg_register();
        gst_plugin_png_register();
        gst_plugin_osxaudio_register();
        gst_plugin_soup_register();
        gst_plugin_hls_register();
        gst_plugin_rtp_register();
        gst_plugin_rtpmanager_register();
        gst_plugin_rtsp_register();
        gst_plugin_udp_register();
        gst_plugin_tcp_register();
        gst_plugin_srtp_register();
        gst_plugin_dtls_register();
        gst_plugin_opengl_register();
        gst_plugin_applemedia_register();
    }
}
