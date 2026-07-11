#include "xhvp_internal.h"

#include <gst/video/video.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static void xhvp_apply_orient_element(GstElement *el, int32_t degrees) {
  if (!el) {
    return;
  }
  if (g_object_class_find_property(G_OBJECT_GET_CLASS(el), "rotation-z")) {
    gfloat z = 0.f;
    switch (degrees) {
    case 90:
      z = -90.f;
      break;
    case 180:
      z = -180.f;
      break;
    case 270:
      z = -270.f;
      break;
    default:
      z = 0.f;
      break;
    }
    g_object_set(el, "rotation-z", z, NULL);
    return;
  }
  if (g_object_class_find_property(G_OBJECT_GET_CLASS(el), "video-direction")) {
    GstVideoOrientationMethod dir = GST_VIDEO_ORIENTATION_IDENTITY;
    switch (degrees) {
    case 90:
      dir = GST_VIDEO_ORIENTATION_90R;
      break;
    case 180:
      dir = GST_VIDEO_ORIENTATION_180;
      break;
    case 270:
      dir = GST_VIDEO_ORIENTATION_90L;
      break;
    default:
      dir = GST_VIDEO_ORIENTATION_IDENTITY;
      break;
    }
    g_object_set(el, "video-direction", dir, NULL);
    return;
  }
  if (g_object_class_find_property(G_OBJECT_GET_CLASS(el), "method")) {
    const char *method = "none";
    switch (degrees) {
    case 90:
      method = "clockwise";
      break;
    case 180:
      method = "rotate-180";
      break;
    case 270:
      method = "counterclockwise";
      break;
    default:
      method = "none";
      break;
    }
    gst_util_set_object_arg(G_OBJECT(el), "method", method);
  }
}

#if !defined(__ANDROID__)
GstElement *xhvp_desktop_make_video_sink(XhvpPlayer *p) {
  GstElement *appsink = gst_element_factory_make("appsink", "xhvp-appsink");
  if (!appsink) {
    return NULL;
  }
  GstCaps *caps = gst_caps_from_string("video/x-raw,format=BGRA");
  g_object_set(appsink, "emit-signals", FALSE, "sync", TRUE, "max-buffers", 2,
               "drop", TRUE, "caps", caps, NULL);
  gst_caps_unref(caps);

  GstAppSinkCallbacks cbs = {
      .eos = NULL,
      .new_preroll = NULL,
      .new_sample = xhvp_frame_on_new_sample,
  };
  gst_app_sink_set_callbacks(GST_APP_SINK(appsink), &cbs, p, NULL);
  p->appsink = appsink;

  GstElement *videoflip =
      gst_element_factory_make("videoflip", "xhvp-videoflip");
  GstElement *convert =
      gst_element_factory_make("videoconvert", "xhvp-vconvert");
  GstElement *capsfilter =
      gst_element_factory_make("capsfilter", "xhvp-bgra-caps");
  if (!videoflip || !convert || !capsfilter) {
    if (videoflip) {
      gst_object_unref(videoflip);
    }
    if (convert) {
      gst_object_unref(convert);
    }
    if (capsfilter) {
      gst_object_unref(capsfilter);
    }
    p->orient_element = NULL;
    return appsink;
  }

  GstCaps *bgra = gst_caps_from_string("video/x-raw,format=BGRA");
  g_object_set(capsfilter, "caps", bgra, NULL);
  gst_caps_unref(bgra);

  GstElement *bin = gst_bin_new("xhvp-video-sink");
  gst_bin_add_many(GST_BIN(bin), videoflip, convert, capsfilter, appsink,
                   NULL);
  if (!gst_element_link_many(videoflip, convert, capsfilter, appsink, NULL)) {
    gst_object_unref(bin);
    p->orient_element = NULL;
    p->appsink = NULL;
    return NULL;
  }
  GstPad *pad = gst_element_get_static_pad(videoflip, "sink");
  GstPad *ghost = gst_ghost_pad_new("sink", pad);
  gst_object_unref(pad);
  gst_element_add_pad(bin, ghost);
  p->orient_element = videoflip;
  xhvp_apply_orient_element(videoflip, p->rotate_degrees);
  return bin;
}
#endif

#if defined(__ANDROID__)
#include <android/native_window.h>

/* Update layout metadata from negotiated video caps and notify Dart.
 * Android has no appsink frames; probe post-glvideoflip so 90/270 report
 * swapped width/height and Dart SurfaceProducer matches the buffer. */
static void xhvp_apply_video_size_from_caps(XhvpPlayer *p, GstCaps *caps) {
  if (!p || !caps || gst_caps_is_empty(caps) || gst_caps_is_any(caps)) {
    return;
  }
  const GstStructure *s = gst_caps_get_structure(caps, 0);
  if (!s) {
    return;
  }
  gint width = 0;
  gint height = 0;
  if (!gst_structure_get_int(s, "width", &width) ||
      !gst_structure_get_int(s, "height", &height) || width <= 0 ||
      height <= 0) {
    return;
  }
  gint par_n = 1;
  gint par_d = 1;
  gst_structure_get_fraction(s, "pixel-aspect-ratio", &par_n, &par_d);
  if (par_n <= 0) {
    par_n = 1;
  }
  if (par_d <= 0) {
    par_d = 1;
  }
  const gint dar_n = width * par_n;
  const gint dar_d = height * par_d;
  if (p->width == width && p->height == height && p->par_n == par_n &&
      p->par_d == par_d && p->dar_n == dar_n && p->dar_d == dar_d) {
    return;
  }
  p->width = width;
  p->height = height;
  p->par_n = par_n;
  p->par_d = par_d;
  p->dar_n = dar_n;
  p->dar_d = dar_d;
  xhvp_player_emit(p, XHVP_EVENT_VIDEO_SIZE, "");
  xhvp_player_emit(p, XHVP_EVENT_METADATA_CHANGED, "");
}

static GstPadProbeReturn xhvp_android_sink_caps_probe(GstPad *pad,
                                                      GstPadProbeInfo *info,
                                                      gpointer user_data) {
  XhvpPlayer *p = user_data;
  if (!p || !p->in_use) {
    return GST_PAD_PROBE_OK;
  }
  if (!(info->type & GST_PAD_PROBE_TYPE_EVENT_DOWNSTREAM)) {
    return GST_PAD_PROBE_OK;
  }
  GstEvent *event = GST_PAD_PROBE_INFO_EVENT(info);
  if (!event || GST_EVENT_TYPE(event) != GST_EVENT_CAPS) {
    return GST_PAD_PROBE_OK;
  }
  GstCaps *caps = NULL;
  gst_event_parse_caps(event, &caps);
  if (caps) {
    xhvp_apply_video_size_from_caps(p, caps);
  }
  (void)pad;
  return GST_PAD_PROBE_OK;
}

static void xhvp_try_update_video_size_from_sink(XhvpPlayer *p) {
  if (!p || !p->pipeline) {
    return;
  }
  /* Prefer post-orient pad (glimagesink) so size reflects glvideoflip swap. */
  GstElement *vsink = NULL;
  g_object_get(p->pipeline, "video-sink", &vsink, NULL);
  if (!vsink) {
    return;
  }
  GstElement *probe_el = NULL;
  if (GST_IS_BIN(vsink)) {
    probe_el = gst_bin_get_by_name(GST_BIN(vsink), "xhvp-glimagesink");
    if (!probe_el) {
      probe_el = gst_bin_get_by_name(GST_BIN(vsink), "xhvp-glvideoflip");
    }
  }
  if (!probe_el) {
    probe_el = gst_object_ref(vsink);
  }
  gst_object_unref(vsink);
  GstPad *sinkpad = gst_element_get_static_pad(probe_el, "sink");
  gst_object_unref(probe_el);
  if (!sinkpad) {
    return;
  }
  GstCaps *caps = gst_pad_get_current_caps(sinkpad);
  if (caps) {
    xhvp_apply_video_size_from_caps(p, caps);
    gst_caps_unref(caps);
  }
  gst_object_unref(sinkpad);
}

GstElement *xhvp_android_make_video_sink(XhvpPlayer *p) {
  /* MediaCodec (amcvideodec) emits GLMemory / external-OES. The sink bin must
   * bridge with glupload → glcolorconvert before glvideoflip/glimagesink.
   * glvideoflip stays on GLMemory, swaps caps for 90/270, and aspect-scales
   * so Dart SurfaceProducer can match post-orient size. Do not insert
   * gldownload / CPU videoconvert / videoflip on this path. */
  GstElement *glupload =
      gst_element_factory_make("glupload", "xhvp-glupload");
  GstElement *glcc =
      gst_element_factory_make("glcolorconvert", "xhvp-glcolorconvert");
  GstElement *glflip =
      gst_element_factory_make("glvideoflip", "xhvp-glvideoflip");
  GstElement *sink =
      gst_element_factory_make("glimagesink", "xhvp-glimagesink");

  if (!glupload || !glcc || !glflip || !sink) {
    if (glupload) {
      gst_object_unref(glupload);
    }
    if (glcc) {
      gst_object_unref(glcc);
    }
    if (glflip) {
      gst_object_unref(glflip);
    }
    if (sink) {
      gst_object_unref(sink);
    }
    p->orient_element = NULL;
    p->overlay_element = NULL;
    return NULL;
  }

  /* Dart FittedBox owns fit/fill/stretch; native must fill the buffer or
   * portrait frames are letterboxed into a landscape SurfaceProducer. */
  g_object_set(sink, "force-aspect-ratio", FALSE, NULL);

  GstElement *bin = gst_bin_new("xhvp-video-sink");
  gst_bin_add_many(GST_BIN(bin), glupload, glcc, glflip, sink, NULL);
  if (!gst_element_link_many(glupload, glcc, glflip, sink, NULL)) {
    gst_object_unref(bin);
    p->orient_element = NULL;
    p->overlay_element = NULL;
    return NULL;
  }

  GstPad *pad = gst_element_get_static_pad(glupload, "sink");
  GstPad *ghost = gst_ghost_pad_new("sink", pad);
  gst_object_unref(pad);
  gst_element_add_pad(bin, ghost);
  p->orient_element = glflip;
  p->overlay_element = GST_IS_VIDEO_OVERLAY(sink) ? sink : NULL;
  xhvp_apply_orient_element(glflip, p->rotate_degrees);

  /* Post-orient pad: size matches glvideoflip output (axes swapped for 90/270). */
  GstPad *sink_pad = gst_element_get_static_pad(sink, "sink");
  if (sink_pad) {
    gst_pad_add_probe(sink_pad, GST_PAD_PROBE_TYPE_EVENT_DOWNSTREAM,
                      xhvp_android_sink_caps_probe, p, NULL);
    gst_object_unref(sink_pad);
  }
  return bin;
}

static GstElement *xhvp_resolve_overlay(XhvpPlayer *p) {
  if (p->overlay_element && GST_IS_VIDEO_OVERLAY(p->overlay_element)) {
    return gst_object_ref(p->overlay_element);
  }
  if (!p->pipeline) {
    return NULL;
  }
  GstElement *sink = NULL;
  g_object_get(p->pipeline, "video-sink", &sink, NULL);
  if (!sink) {
    return NULL;
  }
  if (GST_IS_VIDEO_OVERLAY(sink)) {
    return sink;
  }
  if (GST_IS_BIN(sink)) {
    GstIterator *it = gst_bin_iterate_recurse(GST_BIN(sink));
    GValue item = G_VALUE_INIT;
    GstElement *found = NULL;
    while (gst_iterator_next(it, &item) == GST_ITERATOR_OK) {
      GstElement *child = g_value_get_object(&item);
      if (child && GST_IS_VIDEO_OVERLAY(child)) {
        found = gst_object_ref(child);
        g_value_unset(&item);
        break;
      }
      g_value_unset(&item);
    }
    gst_iterator_free(it);
    gst_object_unref(sink);
    if (found) {
      /* Non-owning cache; bin owns the element. */
      p->overlay_element = found;
      gst_object_unref(found);
      return gst_object_ref(p->overlay_element);
    }
    return NULL;
  }
  gst_object_unref(sink);
  return NULL;
}

void xhvp_android_release_window(XhvpPlayer *p) {
  if (p->android_window == 0) {
    return;
  }
  ANativeWindow *win = (ANativeWindow *)(intptr_t)p->android_window;
  ANativeWindow_release(win);
  p->android_window = 0;
  p->android_w = 0;
  p->android_h = 0;
  p->android_overlay_bound = false;
}

/* Detach VideoOverlay from the current pipeline without releasing the
 * ANativeWindow. SurfaceProducer does not re-fire onSurfaceAvailable on media
 * reload, so the window must survive destroy/load. */
static void xhvp_android_unbind_overlay(XhvpPlayer *p) {
  GstElement *overlay = xhvp_resolve_overlay(p);
  if (overlay) {
    gst_video_overlay_set_window_handle(GST_VIDEO_OVERLAY(overlay), 0);
    gst_object_unref(overlay);
  }
  p->android_overlay_bound = false;
  p->overlay_element = NULL;
}

void xhvp_android_clear_overlay(XhvpPlayer *p) {
  xhvp_android_unbind_overlay(p);
  xhvp_android_release_window(p);
}

void xhvp_android_apply_overlay(XhvpPlayer *p) {
  if (!p->pipeline || p->android_window == 0) {
    return;
  }
  /* Prefer live ANativeWindow size over a stale first-bind cache. */
  ANativeWindow *win = (ANativeWindow *)(intptr_t)p->android_window;
  const int32_t live_w = ANativeWindow_getWidth(win);
  const int32_t live_h = ANativeWindow_getHeight(win);
  if (live_w > 0 && live_h > 0) {
    p->android_w = live_w;
    p->android_h = live_h;
  }
  GstElement *overlay = xhvp_resolve_overlay(p);
  if (overlay) {
    gst_video_overlay_set_window_handle(GST_VIDEO_OVERLAY(overlay),
                                        (guintptr)p->android_window);
    if (p->android_w > 0 && p->android_h > 0) {
      gst_video_overlay_set_render_rectangle(GST_VIDEO_OVERLAY(overlay), 0, 0,
                                             p->android_w, p->android_h);
    }
    /* GStreamer Android tutorial: expose twice so GLES picks up size changes. */
    gst_video_overlay_expose(GST_VIDEO_OVERLAY(overlay));
    gst_video_overlay_expose(GST_VIDEO_OVERLAY(overlay));
    p->android_overlay_bound = true;
    gst_object_unref(overlay);
  }

  if (!(p->pending_auto_play || p->desired_playing)) {
    return;
  }
  /* Do not play before the pipeline has reached PAUSED (load mid-flight). */
  GstState cur = GST_STATE_NULL;
  gst_element_get_state(p->pipeline, &cur, NULL, 0);
  const bool gst_ready =
      (cur == GST_STATE_PAUSED || cur == GST_STATE_PLAYING);
  const bool ui_ready =
      p->player_state == XHVP_STATE_READY ||
      p->player_state == XHVP_STATE_BUFFERING ||
      p->player_state == XHVP_STATE_PLAYING ||
      p->player_state == XHVP_STATE_PAUSED;
  if (!gst_ready && !ui_ready) {
    return;
  }
  p->pending_auto_play = false;
  xhvp_pipeline_play(p);
}
#endif

static void xhvp_reset_media_fields(XhvpPlayer *p) {
  p->duration_ms = 0;
  p->position_ms = 0;
  p->width = 0;
  p->height = 0;
  p->fps = 0;
  p->par_n = 1;
  p->par_d = 1;
  p->dar_n = 0;
  p->dar_d = 0;
  p->interlaced = false;
  p->track_count = 0;
  p->at_eos = false;
  p->seekable = true;
  p->buffering_percent = 100;
  p->pending_rate_seek = false;
  p->rotate_degrees = 0;
  p->color_matrix[0] = '\0';
  p->color_range[0] = '\0';
  p->hdr_format[0] = '\0';
  xhvp_frame_clear(p);
}

static void xhvp_clear_asset_temp(XhvpPlayer *p) {
  if (p->asset_temp_path[0] != '\0') {
    unlink(p->asset_temp_path);
    p->asset_temp_path[0] = '\0';
  }
  if (p->asset_bytes) {
    g_free(p->asset_bytes);
    p->asset_bytes = NULL;
    p->asset_len = 0;
    p->asset_offset = 0;
  }
}

static int32_t xhvp_pipeline_set_state_sync(XhvpPlayer *p, GstState state);

void xhvp_pipeline_destroy(XhvpPlayer *p) {
  xhvp_bus_detach(p);
#if !defined(__ANDROID__)
  if (p->appsink) {
    gst_app_sink_set_callbacks(GST_APP_SINK(p->appsink), NULL, NULL, NULL);
  }
#endif
#if defined(__ANDROID__)
  /* Keep ANativeWindow across reload; only unbind from the old pipeline. */
  xhvp_android_unbind_overlay(p);
#endif
  if (p->pipeline) {
    (void)xhvp_pipeline_set_state_sync(p, GST_STATE_NULL);
    gst_object_unref(p->pipeline);
    p->pipeline = NULL;
  }
  p->appsink = NULL;
  p->appsrc = NULL;
  p->orient_element = NULL;
  p->overlay_element = NULL;
  if (p->stream_collection) {
    gst_object_unref(p->stream_collection);
    p->stream_collection = NULL;
  }
  p->buffering_percent = 100;
  p->pending_rate_seek = false;
  xhvp_clear_asset_temp(p);
}

static bool xhvp_pipeline_usable_after_failure(XhvpPlayer *p) {
  if (!p->pipeline) {
    return false;
  }
  GstState cur = GST_STATE_NULL;
  gst_element_get_state(p->pipeline, &cur, NULL, 0);
  if (cur == GST_STATE_PAUSED || cur == GST_STATE_PLAYING) {
    return true;
  }
  /* Autoplug can leave GstState at READY while duration/UI already advanced. */
  if (p->duration_ms > 0) {
    return true;
  }
  if (p->player_state == XHVP_STATE_BUFFERING ||
      p->player_state == XHVP_STATE_PLAYING ||
      p->player_state == XHVP_STATE_PAUSED ||
      p->player_state == XHVP_STATE_READY) {
    return true;
  }
  return false;
}

static int32_t xhvp_pipeline_set_state_sync(XhvpPlayer *p, GstState state) {
  if (!p->pipeline) {
    return XHVP_ERR_NOT_READY;
  }
  GstStateChangeReturn ret = gst_element_set_state(p->pipeline, state);
  if (ret == GST_STATE_CHANGE_FAILURE) {
    return xhvp_pipeline_usable_after_failure(p) ? XHVP_ERR_OK : XHVP_ERR_FAIL;
  }
  if (ret == GST_STATE_CHANGE_ASYNC) {
    ret = gst_element_get_state(p->pipeline, NULL, NULL, 5 * GST_SECOND);
    if (ret == GST_STATE_CHANGE_FAILURE) {
      return xhvp_pipeline_usable_after_failure(p) ? XHVP_ERR_OK
                                                   : XHVP_ERR_FAIL;
    }
  }
  return XHVP_ERR_OK;
}

static GstElement *xhvp_make_playbin(void) {
  GstElement *pipeline = gst_element_factory_make("playbin3", "xhvp-playbin");
  if (!pipeline) {
    pipeline = gst_element_factory_make("playbin", "xhvp-playbin");
  }
  return pipeline;
}

/* Relax HTTPS cert checks and set a browser-like UA for souphttpsrc (and
 * similar HTTP sources). Required on iOS static GStreamer where the bundled
 * trust store is minimal; harmless on other platforms. */
static void xhvp_configure_http_source(GstElement *element) {
  if (!element) {
    return;
  }
  GObjectClass *klass = G_OBJECT_GET_CLASS(element);
  if (g_object_class_find_property(klass, "ssl-strict")) {
    g_object_set(element, "ssl-strict", FALSE, NULL);
  }
  if (g_object_class_find_property(klass, "tls-validation-flags")) {
    g_object_set(element, "tls-validation-flags", 0, NULL);
  }
  if (g_object_class_find_property(klass, "user-agent")) {
    g_object_set(element, "user-agent",
                 "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) "
                 "AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 "
                 "Mobile/15E148 Safari/604.1",
                 NULL);
  }
}

static void xhvp_on_source_setup(GstElement *playbin, GstElement *source,
                                 gpointer user_data) {
  (void)playbin;
  (void)user_data;
  xhvp_configure_http_source(source);
}

static void xhvp_on_element_setup(GstElement *bin, GstElement *element,
                                  gpointer user_data) {
  (void)bin;
  (void)user_data;
  xhvp_configure_http_source(element);
}

static void xhvp_attach_http_source_handlers(GstElement *pipeline) {
  if (!pipeline) {
    return;
  }
  g_signal_connect(pipeline, "source-setup", G_CALLBACK(xhvp_on_source_setup),
                   NULL);
  /* playbin3 / urisourcebin may create souphttpsrc as a nested child. */
  g_signal_connect(pipeline, "element-setup", G_CALLBACK(xhvp_on_element_setup),
                   NULL);
}

static void xhvp_attach_video_sink(XhvpPlayer *p, GstElement *pipeline) {
  p->orient_element = NULL;
#if defined(__ANDROID__)
  GstElement *vsink = xhvp_android_make_video_sink(p);
#else
  GstElement *vsink = xhvp_desktop_make_video_sink(p);
#endif
  if (vsink) {
    g_object_set(pipeline, "video-sink", vsink, NULL);
  }
}

/* Pitch-preserving rate changes via scaletempo; fall back to default sink. */
static void xhvp_attach_audio_sink(GstElement *pipeline) {
  GstElement *scaletempo =
      gst_element_factory_make("scaletempo", "xhvp-scaletempo");
  GstElement *convert =
      gst_element_factory_make("audioconvert", "xhvp-aconvert");
  GstElement *resample =
      gst_element_factory_make("audioresample", "xhvp-aresample");
  GstElement *sink =
      gst_element_factory_make("autoaudiosink", "xhvp-asink");
  if (!scaletempo || !convert || !resample || !sink) {
    if (scaletempo) {
      gst_object_unref(scaletempo);
    }
    if (convert) {
      gst_object_unref(convert);
    }
    if (resample) {
      gst_object_unref(resample);
    }
    if (sink) {
      gst_object_unref(sink);
    }
    return;
  }

  GstElement *bin = gst_bin_new("xhvp-audio-sink");
  gst_bin_add_many(GST_BIN(bin), scaletempo, convert, resample, sink, NULL);
  if (!gst_element_link_many(scaletempo, convert, resample, sink, NULL)) {
    gst_object_unref(bin);
    return;
  }
  GstPad *pad = gst_element_get_static_pad(scaletempo, "sink");
  GstPad *ghost = gst_ghost_pad_new("sink", pad);
  gst_object_unref(pad);
  gst_element_add_pad(bin, ghost);
  g_object_set(pipeline, "audio-sink", bin, NULL);
}

#if defined(__ANDROID__)
typedef struct {
  XhvpPlayerId id;
} XhvpDeferredPlay;

static gboolean xhvp_deferred_play_cb(gpointer data) {
  XhvpDeferredPlay *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (p && p->pipeline && (p->pending_auto_play || p->desired_playing)) {
    p->pending_auto_play = false;
    (void)xhvp_pipeline_play(p);
  }
  g_free(op);
  return G_SOURCE_REMOVE;
}
#endif

int32_t xhvp_pipeline_load_uri(XhvpPlayer *p, const char *uri, bool auto_play) {
  if (!uri || !*uri) {
    return XHVP_ERR_FAIL;
  }
  xhvp_pipeline_destroy(p);
  xhvp_reset_media_fields(p);
  /* Drop stale UI state so early apply_overlay does not play before PAUSED. */
  xhvp_player_set_state(p, XHVP_STATE_IDLE);
  p->is_uri = true;
  p->desired_playing = auto_play;
  p->pending_auto_play = auto_play;

  GstElement *pipeline = xhvp_make_playbin();
  if (!pipeline) {
    return XHVP_ERR_FAIL;
  }

  g_object_set(pipeline, "uri", uri, NULL);
  xhvp_attach_http_source_handlers(pipeline);
  xhvp_attach_video_sink(p, pipeline);
  xhvp_attach_audio_sink(pipeline);

  p->pipeline = pipeline;
  g_object_set(pipeline, "volume", p->volume, NULL);
  g_object_set(pipeline, "mute", p->muted, NULL);

  xhvp_bus_attach(p);

#if defined(__ANDROID__)
  if (p->android_window != 0) {
    xhvp_android_apply_overlay(p);
  }
#endif

  int32_t rc = xhvp_pipeline_set_state_sync(p, GST_STATE_PAUSED);
#if defined(__ANDROID__)
  if (rc != XHVP_ERR_OK) {
    /* get_state blocks the GST thread so bus watches may not have run yet.
     * Drain pending sources, then re-check; still return OK if the pipeline
     * exists so Dart does not tear down a usable session. */
    XhvpRuntime *rt = xhvp_runtime();
    if (rt && rt->ctx) {
      while (g_main_context_iteration(rt->ctx, FALSE)) {
      }
    }
    if (xhvp_pipeline_usable_after_failure(p) || p->pipeline != NULL) {
      rc = XHVP_ERR_OK;
    }
  }
  if (rc != XHVP_ERR_OK) {
    return rc;
  }

  xhvp_player_set_state(p, XHVP_STATE_READY);
  /* Caps may already be negotiated after preroll; emit size so Dart layout
   * does not stay on the 16:9 fallback until the next CAPS event. */
  xhvp_try_update_video_size_from_sink(p);

  /* Do not return play()'s result as load failure: buffering/autoplug can
   * make set_state(PLAYING) report FAILURE while the pipeline is usable. */
  if (auto_play) {
    p->desired_playing = true;
    p->pending_auto_play = true;
    if (p->android_window != 0) {
      if (!p->android_overlay_bound) {
        xhvp_android_apply_overlay(p);
      }
      XhvpDeferredPlay *op = g_new(XhvpDeferredPlay, 1);
      op->id = p->id;
      xhvp_runtime_invoke_async(xhvp_deferred_play_cb, op);
    }
  }
  return XHVP_ERR_OK;
#else
  if (rc != XHVP_ERR_OK) {
    return rc;
  }

  xhvp_player_set_state(p, XHVP_STATE_READY);

  if (auto_play) {
    return xhvp_pipeline_play(p);
  }
  return XHVP_ERR_OK;
#endif
}

int32_t xhvp_pipeline_load_asset(XhvpPlayer *p, const uint8_t *bytes,
                                 uint32_t len, bool auto_play) {
  if (!bytes || len == 0) {
    return XHVP_ERR_FAIL;
  }

  /* Write bytes to a temp file and play via playbin (same path as URI).
   * GLib requires XXXXXX at the end of the template (no suffix after it). */
  gchar *tmp_path = NULL;
  gint fd = g_file_open_tmp("xhvp-asset-XXXXXX", &tmp_path, NULL);
  if (fd < 0 || !tmp_path) {
    g_free(tmp_path);
    return XHVP_ERR_FAIL;
  }
  gssize written = write(fd, bytes, len);
  close(fd);
  if (written < 0 || (uint32_t)written != len) {
    unlink(tmp_path);
    g_free(tmp_path);
    return XHVP_ERR_FAIL;
  }

  gchar *file_uri = g_filename_to_uri(tmp_path, NULL, NULL);
  if (!file_uri) {
    unlink(tmp_path);
    g_free(tmp_path);
    return XHVP_ERR_FAIL;
  }

  int32_t rc = xhvp_pipeline_load_uri(p, file_uri, auto_play);
  g_free(file_uri);
  if (rc != XHVP_ERR_OK) {
    /* If the pipeline was created and may still be reading the file, keep the
     * temp path for destroy — unlinking now causes "Internal data stream
     * error". */
    if (p->pipeline != NULL) {
      strncpy(p->asset_temp_path, tmp_path, sizeof(p->asset_temp_path) - 1);
      p->asset_temp_path[sizeof(p->asset_temp_path) - 1] = '\0';
      g_free(tmp_path);
    } else {
      unlink(tmp_path);
      g_free(tmp_path);
    }
    return rc;
  }

  p->is_uri = false;
  strncpy(p->asset_temp_path, tmp_path, sizeof(p->asset_temp_path) - 1);
  p->asset_temp_path[sizeof(p->asset_temp_path) - 1] = '\0';
  g_free(tmp_path);
  return XHVP_ERR_OK;
}

int32_t xhvp_pipeline_play(XhvpPlayer *p) {
  if (!p->pipeline) {
    return XHVP_ERR_NOT_READY;
  }
  /* Manual replay after EOS must restart from the beginning (looping already
   * seeks on EOS; play() alone would resume near the end). */
  const bool near_end =
      p->duration_ms > 0 && p->position_ms >= p->duration_ms - 50;
  if (p->at_eos || near_end) {
    int32_t seek_rc = xhvp_pipeline_seek(p, 0);
    if (seek_rc != XHVP_ERR_OK) {
      return seek_rc;
    }
  }
  p->desired_playing = true;
  p->at_eos = false;
#if defined(__ANDROID__)
  if (p->android_window != 0 && !p->android_overlay_bound) {
    xhvp_android_apply_overlay(p);
  }
  /* No ANativeWindow → glimagesink blocks the whole playbin (including audio). */
  if (p->android_window == 0) {
    p->pending_auto_play = true;
    return XHVP_ERR_OK;
  }
#endif
  if (p->speed != 1.0 && p->speed > 0) {
    (void)xhvp_pipeline_apply_rate(p);
  }
  int32_t rc = xhvp_pipeline_set_state_sync(p, GST_STATE_PLAYING);
  if (rc == XHVP_ERR_OK) {
    xhvp_player_set_state(p, XHVP_STATE_PLAYING);
  }
  return rc;
}

int32_t xhvp_pipeline_pause(XhvpPlayer *p) {
  p->desired_playing = false;
  p->pending_auto_play = false;
  int32_t rc = xhvp_pipeline_set_state_sync(p, GST_STATE_PAUSED);
  if (rc == XHVP_ERR_OK) {
    xhvp_player_set_state(p, XHVP_STATE_PAUSED);
  }
  return rc;
}

int32_t xhvp_pipeline_stop(XhvpPlayer *p) {
  p->desired_playing = false;
  p->pending_auto_play = false;
  int32_t rc = xhvp_pipeline_set_state_sync(p, GST_STATE_NULL);
  if (rc == XHVP_ERR_OK) {
    p->position_ms = 0;
    xhvp_player_set_state(p, XHVP_STATE_STOPPED);
  }
  return rc;
}

int32_t xhvp_pipeline_seek(XhvpPlayer *p, int64_t position_ms) {
  if (!p->pipeline) {
    return XHVP_ERR_NOT_READY;
  }
  gint64 pos = position_ms * GST_MSECOND;
  gboolean ok = gst_element_seek(
      p->pipeline, p->speed > 0 ? p->speed : 1.0, GST_FORMAT_TIME,
      (GstSeekFlags)(GST_SEEK_FLAG_FLUSH | GST_SEEK_FLAG_KEY_UNIT),
      GST_SEEK_TYPE_SET, pos, GST_SEEK_TYPE_NONE, GST_CLOCK_TIME_NONE);
  if (!ok) {
    return XHVP_ERR_FAIL;
  }
  p->position_ms = position_ms;
  p->at_eos = false;
  /* Local/fully-buffered seeks often emit no BUFFERING messages; clear any
   * sticky buffering so Dart does not stay on the loading overlay. */
  p->buffering_percent = 100;
  xhvp_player_emit(p, XHVP_EVENT_POSITION_CHANGED, "");
  xhvp_player_emit(p, XHVP_EVENT_BUFFERING, "");
  if (p->desired_playing) {
    xhvp_player_set_state(p, XHVP_STATE_PLAYING);
  } else if (p->player_state == XHVP_STATE_BUFFERING) {
    xhvp_player_set_state(p, XHVP_STATE_PAUSED);
  }
  return XHVP_ERR_OK;
}

int32_t xhvp_pipeline_set_volume(XhvpPlayer *p, double volume) {
  p->volume = volume < 0 ? 0 : (volume > 1 ? 1 : volume);
  if (p->pipeline) {
    g_object_set(p->pipeline, "volume", p->volume, NULL);
  }
  return XHVP_ERR_OK;
}

int32_t xhvp_pipeline_set_mute(XhvpPlayer *p, bool mute) {
  p->muted = mute;
  if (p->pipeline) {
    g_object_set(p->pipeline, "mute", mute, NULL);
  }
  return XHVP_ERR_OK;
}

int32_t xhvp_pipeline_apply_rate(XhvpPlayer *p) {
  if (!p->pipeline) {
    return XHVP_ERR_NOT_READY;
  }
  double speed = p->speed > 0 ? p->speed : 1.0;

  /* Flushing rate seek with an explicit start position. Do not use
   * SEEK_TYPE_NONE+FLUSH (playbin3 can return TRUE while landing at EOS) or
   * INSTANT_RATE_CHANGE (can leave videoconvert/appsink with wrong colors). */
  gint64 pos = GST_CLOCK_TIME_NONE;
  if (!gst_element_query_position(p->pipeline, GST_FORMAT_TIME, &pos) ||
      !GST_CLOCK_TIME_IS_VALID(pos)) {
    pos = (gint64)p->position_ms * GST_MSECOND;
  }
  gboolean ok = gst_element_seek(
      p->pipeline, speed, GST_FORMAT_TIME,
      (GstSeekFlags)(GST_SEEK_FLAG_FLUSH | GST_SEEK_FLAG_KEY_UNIT),
      GST_SEEK_TYPE_SET, pos, GST_SEEK_TYPE_NONE, GST_CLOCK_TIME_NONE);
  if (!ok) {
    return XHVP_ERR_FAIL;
  }

  gint64 after = GST_CLOCK_TIME_NONE;
  if (gst_element_query_position(p->pipeline, GST_FORMAT_TIME, &after) &&
      GST_CLOCK_TIME_IS_VALID(after)) {
    p->position_ms = (int64_t)(after / GST_MSECOND);
  } else {
    p->position_ms = (int64_t)(pos / GST_MSECOND);
  }
  p->at_eos = false;
  xhvp_player_emit(p, XHVP_EVENT_POSITION_CHANGED, "");
  return XHVP_ERR_OK;
}

int32_t xhvp_pipeline_set_speed(XhvpPlayer *p, double speed) {
  if (speed <= 0) {
    speed = 1.0;
  }
  p->speed = speed;
  if (!p->pipeline) {
    p->pending_rate_seek = false;
    return XHVP_ERR_OK;
  }
  /* Avoid fighting the buffering pause/resume loop with a mid-rebuffer seek. */
  if (p->buffering_percent < 100) {
    p->pending_rate_seek = true;
    return XHVP_ERR_OK;
  }
  p->pending_rate_seek = false;
  return xhvp_pipeline_apply_rate(p);
}

void xhvp_pipeline_refresh_tracks(XhvpPlayer *p) {
  p->track_count = 0;
  if (!p->pipeline) {
    return;
  }

  /* playbin3: prefer GstStreamCollection (no n-audio / current-audio). */
  if (p->stream_collection) {
    guint n = gst_stream_collection_get_size(p->stream_collection);
    for (guint i = 0; i < n && p->track_count < XHVP_MAX_TRACKS; i++) {
      GstStream *stream =
          gst_stream_collection_get_stream(p->stream_collection, i);
      if (!stream) {
        continue;
      }
      GstStreamType stype = gst_stream_get_stream_type(stream);
      int32_t track_type = -1;
      const char *prefix = NULL;
      if (stype & GST_STREAM_TYPE_AUDIO) {
        track_type = XHVP_TRACK_AUDIO;
        prefix = "Audio";
      } else if (stype & GST_STREAM_TYPE_VIDEO) {
        track_type = XHVP_TRACK_VIDEO;
        prefix = "Video";
      } else if (stype & GST_STREAM_TYPE_TEXT) {
        track_type = XHVP_TRACK_SUBTITLE;
        prefix = "Subtitle";
      } else {
        continue;
      }

      XhvpTrackInfo *t = &p->tracks[p->track_count];
      t->id = (int32_t)p->track_count;
      t->type = track_type;
      t->selected = false;
      t->language[0] = '\0';
      t->stream_id[0] = '\0';
      snprintf(t->label, sizeof(t->label), "%s %d", prefix, t->id);
      {
        const gchar *sid = gst_stream_get_stream_id(stream);
        if (sid) {
          strncpy(t->stream_id, sid, sizeof(t->stream_id) - 1);
          t->stream_id[sizeof(t->stream_id) - 1] = '\0';
        }
      }

      GstCaps *caps = gst_stream_get_caps(stream);
      if (caps && !gst_caps_is_empty(caps)) {
        const GstStructure *s = gst_caps_get_structure(caps, 0);
        const gchar *lang = gst_structure_get_string(s, "language");
        if (!lang) {
          lang = gst_structure_get_string(s, "lang");
        }
        if (lang) {
          strncpy(t->language, lang, sizeof(t->language) - 1);
          t->language[sizeof(t->language) - 1] = '\0';
        }
      }
      if (caps) {
        gst_caps_unref(caps);
      }
      GstTagList *tags = gst_stream_get_tags(stream);
      if (tags) {
        gchar *lang = NULL;
        if (gst_tag_list_get_string(tags, GST_TAG_LANGUAGE_CODE, &lang) &&
            lang) {
          strncpy(t->language, lang, sizeof(t->language) - 1);
          t->language[sizeof(t->language) - 1] = '\0';
          g_free(lang);
        }
        gst_tag_list_unref(tags);
      }
      p->track_count++;
    }
    return;
  }

  /* playbin2 fallback: only query when the property exists. */
  GObjectClass *klass = G_OBJECT_GET_CLASS(p->pipeline);
  if (!g_object_class_find_property(klass, "n-audio")) {
    return;
  }

  gint n_audio = 0, n_video = 0, n_text = 0;
  g_object_get(p->pipeline, "n-audio", &n_audio, "n-video", &n_video, "n-text",
               &n_text, NULL);
  gint cur_audio = -1, cur_video = -1, cur_text = -1;
  g_object_get(p->pipeline, "current-audio", &cur_audio, "current-video",
               &cur_video, "current-text", &cur_text, NULL);

  for (gint i = 0; i < n_audio && p->track_count < XHVP_MAX_TRACKS; i++) {
    XhvpTrackInfo *t = &p->tracks[p->track_count++];
    t->id = i;
    t->type = XHVP_TRACK_AUDIO;
    snprintf(t->label, sizeof(t->label), "Audio %d", i);
    t->language[0] = '\0';
    t->stream_id[0] = '\0';
    t->selected = (i == cur_audio);
  }
  for (gint i = 0; i < n_video && p->track_count < XHVP_MAX_TRACKS; i++) {
    XhvpTrackInfo *t = &p->tracks[p->track_count++];
    t->id = i;
    t->type = XHVP_TRACK_VIDEO;
    snprintf(t->label, sizeof(t->label), "Video %d", i);
    t->language[0] = '\0';
    t->stream_id[0] = '\0';
    t->selected = (i == cur_video);
  }
  for (gint i = 0; i < n_text && p->track_count < XHVP_MAX_TRACKS; i++) {
    XhvpTrackInfo *t = &p->tracks[p->track_count++];
    t->id = i;
    t->type = XHVP_TRACK_SUBTITLE;
    snprintf(t->label, sizeof(t->label), "Subtitle %d", i);
    t->language[0] = '\0';
    t->stream_id[0] = '\0';
    t->selected = (i == cur_text);
  }
}

void xhvp_pipeline_apply_streams_selected(XhvpPlayer *p, GstMessage *msg) {
  if (!p || !msg) {
    return;
  }
  for (int i = 0; i < p->track_count; i++) {
    p->tracks[i].selected = false;
  }
  guint n = gst_message_streams_selected_get_size(msg);
  for (guint i = 0; i < n; i++) {
    GstStream *stream = gst_message_streams_selected_get_stream(msg, i);
    if (!stream) {
      continue;
    }
    const gchar *sid = gst_stream_get_stream_id(stream);
    if (!sid) {
      continue;
    }
    for (int t = 0; t < p->track_count; t++) {
      if (p->tracks[t].stream_id[0] != '\0' &&
          strcmp(p->tracks[t].stream_id, sid) == 0) {
        p->tracks[t].selected = true;
        break;
      }
    }
  }
}

void xhvp_pipeline_update_seekable(XhvpPlayer *p) {
  if (!p || !p->pipeline) {
    return;
  }
  GstQuery *query = gst_query_new_seeking(GST_FORMAT_TIME);
  if (!query) {
    return;
  }
  if (gst_element_query(p->pipeline, query)) {
    gboolean seekable = FALSE;
    gst_query_parse_seeking(query, NULL, &seekable, NULL, NULL);
    p->seekable = seekable ? true : false;
  }
  gst_query_unref(query);
}

static int32_t xhvp_stream_type_to_track(GstStreamType stype) {
  if (stype & GST_STREAM_TYPE_AUDIO) {
    return XHVP_TRACK_AUDIO;
  }
  if (stype & GST_STREAM_TYPE_VIDEO) {
    return XHVP_TRACK_VIDEO;
  }
  if (stype & GST_STREAM_TYPE_TEXT) {
    return XHVP_TRACK_SUBTITLE;
  }
  return -1;
}

int32_t xhvp_pipeline_select_track(XhvpPlayer *p, int32_t track_id,
                                   int32_t track_type, bool enable) {
  if (!p->pipeline) {
    return XHVP_ERR_FAIL;
  }
  if (!enable) {
    return XHVP_ERR_OK;
  }

  /* playbin3: select by stream-id from the cached collection. */
  if (p->stream_collection) {
    if (track_id < 0 || track_id >= p->track_count) {
      return XHVP_ERR_FAIL;
    }
    guint n = gst_stream_collection_get_size(p->stream_collection);
    GList *ids = NULL;
    gboolean have_audio = FALSE, have_video = FALSE, have_text = FALSE;
    gint mapped = -1;
    for (guint i = 0; i < n; i++) {
      GstStream *stream =
          gst_stream_collection_get_stream(p->stream_collection, i);
      if (!stream) {
        continue;
      }
      int32_t tt = xhvp_stream_type_to_track(gst_stream_get_stream_type(stream));
      if (tt < 0) {
        continue;
      }
      mapped++;
      const gchar *sid = gst_stream_get_stream_id(stream);
      if (!sid) {
        continue;
      }
      if (tt == track_type) {
        if (mapped == track_id) {
          ids = g_list_append(ids, g_strdup(sid));
          if (tt == XHVP_TRACK_AUDIO) {
            have_audio = TRUE;
          } else if (tt == XHVP_TRACK_VIDEO) {
            have_video = TRUE;
          } else {
            have_text = TRUE;
          }
        }
        continue;
      }
      if (tt == XHVP_TRACK_AUDIO && !have_audio) {
        ids = g_list_append(ids, g_strdup(sid));
        have_audio = TRUE;
      } else if (tt == XHVP_TRACK_VIDEO && !have_video) {
        ids = g_list_append(ids, g_strdup(sid));
        have_video = TRUE;
      } else if (tt == XHVP_TRACK_SUBTITLE && !have_text) {
        ids = g_list_append(ids, g_strdup(sid));
        have_text = TRUE;
      }
    }
    if (!ids) {
      return XHVP_ERR_FAIL;
    }
    gboolean ok =
        gst_element_send_event(p->pipeline, gst_event_new_select_streams(ids));
    g_list_free_full(ids, g_free);
    if (!ok) {
      return XHVP_ERR_FAIL;
    }
    for (int i = 0; i < p->track_count; i++) {
      if (p->tracks[i].type == track_type) {
        p->tracks[i].selected = (p->tracks[i].id == track_id);
      }
    }
    xhvp_player_emit(p, XHVP_EVENT_TRACKS_CHANGED, "");
    return XHVP_ERR_OK;
  }

  GObjectClass *klass = G_OBJECT_GET_CLASS(p->pipeline);
  if (!g_object_class_find_property(klass, "current-audio")) {
    return XHVP_ERR_FAIL;
  }
  switch (track_type) {
  case XHVP_TRACK_AUDIO:
    g_object_set(p->pipeline, "current-audio", track_id, NULL);
    break;
  case XHVP_TRACK_VIDEO:
    g_object_set(p->pipeline, "current-video", track_id, NULL);
    break;
  case XHVP_TRACK_SUBTITLE:
    g_object_set(p->pipeline, "current-text", track_id, NULL);
    break;
  default:
    return XHVP_ERR_FAIL;
  }
  xhvp_pipeline_refresh_tracks(p);
  xhvp_player_emit(p, XHVP_EVENT_TRACKS_CHANGED, "");
  return XHVP_ERR_OK;
}

int32_t xhvp_pipeline_set_rotation(XhvpPlayer *p, int32_t degrees) {
  if (degrees != 0 && degrees != 90 && degrees != 180 && degrees != 270) {
    return XHVP_ERR_FAIL;
  }
  const int32_t prev = p->rotate_degrees;
  p->rotate_degrees = degrees;
  xhvp_apply_orient_element(p->orient_element, degrees);

  /* Eagerly swap layout metadata for 90/270 so Dart letterboxes before the
   * next post-orient caps/frame (videoflip / glvideoflip swap axes). */
  const bool prev_swap = (prev == 90 || prev == 270);
  const bool next_swap = (degrees == 90 || degrees == 270);
  if (prev_swap != next_swap && p->width > 0 && p->height > 0) {
    int32_t tmp = p->width;
    p->width = p->height;
    p->height = tmp;
    if (p->dar_n > 0 && p->dar_d > 0) {
      tmp = p->dar_n;
      p->dar_n = p->dar_d;
      p->dar_d = tmp;
    } else {
      p->dar_n = p->width;
      p->dar_d = p->height;
    }
    xhvp_player_emit(p, XHVP_EVENT_VIDEO_SIZE, "");
    xhvp_player_emit(p, XHVP_EVENT_METADATA_CHANGED, "");
  }
  return XHVP_ERR_OK;
}

int32_t xhvp_pipeline_set_aspect(XhvpPlayer *p, int32_t mode) {
  if (mode < 0 || mode > 2) {
    return XHVP_ERR_FAIL;
  }
  /* Stored for API compatibility. Layout (fit/fill/stretch) is owned by Dart
   * FittedBox; Android glimagesink keeps force-aspect-ratio=false. */
  p->aspect_mode = mode;
  return XHVP_ERR_OK;
}
