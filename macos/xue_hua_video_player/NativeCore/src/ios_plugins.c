#include "xhvp_internal.h"

#if defined(__APPLE__)
#include <TargetConditionals.h>
#endif

#if defined(TARGET_OS_IPHONE) && TARGET_OS_IPHONE

#define XHVP_DECL_PLUGIN(name) void gst_plugin_##name##_register(void)

XHVP_DECL_PLUGIN(coreelements);
XHVP_DECL_PLUGIN(app);
XHVP_DECL_PLUGIN(typefindfunctions);
XHVP_DECL_PLUGIN(playback);
XHVP_DECL_PLUGIN(autodetect);
XHVP_DECL_PLUGIN(pbtypes);
XHVP_DECL_PLUGIN(gio);
XHVP_DECL_PLUGIN(videoconvertscale);
XHVP_DECL_PLUGIN(videofilter);
XHVP_DECL_PLUGIN(videorate);
XHVP_DECL_PLUGIN(deinterlace);
XHVP_DECL_PLUGIN(videocrop);
XHVP_DECL_PLUGIN(audioconvert);
XHVP_DECL_PLUGIN(audioresample);
XHVP_DECL_PLUGIN(audiorate);
XHVP_DECL_PLUGIN(volume);
XHVP_DECL_PLUGIN(audiofx);
XHVP_DECL_PLUGIN(audioparsers);
XHVP_DECL_PLUGIN(videoparsersbad);
XHVP_DECL_PLUGIN(isomp4);
XHVP_DECL_PLUGIN(matroska);
XHVP_DECL_PLUGIN(id3demux);
XHVP_DECL_PLUGIN(subparse);
XHVP_DECL_PLUGIN(libav);
XHVP_DECL_PLUGIN(jpeg);
XHVP_DECL_PLUGIN(png);
XHVP_DECL_PLUGIN(osxaudio);
XHVP_DECL_PLUGIN(soup);
XHVP_DECL_PLUGIN(hls);
XHVP_DECL_PLUGIN(rtp);
XHVP_DECL_PLUGIN(rtpmanager);
XHVP_DECL_PLUGIN(rtsp);
XHVP_DECL_PLUGIN(udp);
XHVP_DECL_PLUGIN(tcp);
XHVP_DECL_PLUGIN(srtp);
XHVP_DECL_PLUGIN(dtls);
XHVP_DECL_PLUGIN(opengl);
XHVP_DECL_PLUGIN(applemedia);

void xhvp_register_ios_static_plugins(void) {
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

#endif
