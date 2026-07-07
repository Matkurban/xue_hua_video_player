// On iOS, GStreamer ships as a single *static* `GStreamer.framework`. As on
// Android, statically-linked plugins are not discovered by scanning the
// filesystem, so each plugin must be registered explicitly.

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

/// Registers the statically-linked GStreamer plugins bundled in the iOS framework.
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
