#include "xhvp_internal.h"

#include <gst/app/gstappsink.h>
#include <gst/video/video.h>
#include <string.h>

#ifndef GST_PLAY_FLAG_VIDEO
#define GST_PLAY_FLAG_VIDEO (1 << 0)
#endif

typedef struct {
  const char *uri;
  int64_t position_ms;
  int32_t max_width;
  uint8_t **out_bgra;
  uint32_t *out_len;
  int32_t *out_width;
  int32_t *out_height;
  int32_t *out_stride;
  int32_t result;
} XhvpThumbnailOp;

static void xhvp_thumb_configure_http(GstElement *element) {
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

static void xhvp_thumb_on_source_setup(GstElement *playbin, GstElement *source,
                                       gpointer user_data) {
  (void)playbin;
  (void)user_data;
  xhvp_thumb_configure_http(source);
}

static void xhvp_thumb_on_element_setup(GstElement *bin, GstElement *element,
                                        gpointer user_data) {
  (void)bin;
  (void)user_data;
  xhvp_thumb_configure_http(element);
}

static GstElement *xhvp_thumb_make_appsink(int32_t max_width) {
  GstElement *appsink = gst_element_factory_make("appsink", "xhvp-thumb-sink");
  if (!appsink) {
    return NULL;
  }
  gchar *caps_str =
      g_strdup_printf("video/x-raw,format=BGRA,width=(int)[1,%d],"
                      "pixel-aspect-ratio=1/1",
                      max_width);
  GstCaps *caps = gst_caps_from_string(caps_str);
  g_free(caps_str);
  g_object_set(appsink, "emit-signals", FALSE, "sync", FALSE, "max-buffers", 1,
               "drop", TRUE, "caps", caps, NULL);
  gst_caps_unref(caps);
  return appsink;
}

static GstElement *xhvp_thumb_make_video_sink(int32_t max_width) {
  GstElement *appsink = xhvp_thumb_make_appsink(max_width);
  if (!appsink) {
    return NULL;
  }

  GstElement *convert =
      gst_element_factory_make("videoconvert", "xhvp-thumb-convert");
  GstElement *scale =
      gst_element_factory_make("videoscale", "xhvp-thumb-scale");
  GstElement *capsfilter =
      gst_element_factory_make("capsfilter", "xhvp-thumb-caps");
  if (!convert || !scale || !capsfilter) {
    if (convert) {
      gst_object_unref(convert);
    }
    if (scale) {
      gst_object_unref(scale);
    }
    if (capsfilter) {
      gst_object_unref(capsfilter);
    }
    gst_object_unref(appsink);
    return NULL;
  }

  gchar *caps_str =
      g_strdup_printf("video/x-raw,format=BGRA,width=(int)[1,%d],"
                      "pixel-aspect-ratio=1/1",
                      max_width);
  GstCaps *caps = gst_caps_from_string(caps_str);
  g_free(caps_str);
  g_object_set(capsfilter, "caps", caps, NULL);
  gst_caps_unref(caps);

  GstElement *bin = gst_bin_new("xhvp-thumb-vsink");
#if defined(__ANDROID__)
  GstElement *glupload =
      gst_element_factory_make("glupload", "xhvp-thumb-glupload");
  GstElement *glcc =
      gst_element_factory_make("glcolorconvert", "xhvp-thumb-glcc");
  GstElement *gldownload =
      gst_element_factory_make("gldownload", "xhvp-thumb-gldownload");
  if (!glupload || !glcc || !gldownload) {
    if (glupload) {
      gst_object_unref(glupload);
    }
    if (glcc) {
      gst_object_unref(glcc);
    }
    if (gldownload) {
      gst_object_unref(gldownload);
    }
    gst_object_unref(bin);
    gst_object_unref(convert);
    gst_object_unref(scale);
    gst_object_unref(capsfilter);
    gst_object_unref(appsink);
    return NULL;
  }
  gst_bin_add_many(GST_BIN(bin), glupload, glcc, gldownload, convert, scale,
                   capsfilter, appsink, NULL);
  if (!gst_element_link_many(glupload, glcc, gldownload, convert, scale,
                             capsfilter, appsink, NULL)) {
    gst_object_unref(bin);
    return NULL;
  }
  GstPad *pad = gst_element_get_static_pad(glupload, "sink");
#else
  gst_bin_add_many(GST_BIN(bin), convert, scale, capsfilter, appsink, NULL);
  if (!gst_element_link_many(convert, scale, capsfilter, appsink, NULL)) {
    gst_object_unref(bin);
    return NULL;
  }
  GstPad *pad = gst_element_get_static_pad(convert, "sink");
#endif
  GstPad *ghost = gst_ghost_pad_new("sink", pad);
  gst_object_unref(pad);
  gst_element_add_pad(bin, ghost);
  return bin;
}

static int32_t xhvp_thumb_copy_sample(GstSample *sample, uint8_t **out_bgra,
                                      uint32_t *out_len, int32_t *out_width,
                                      int32_t *out_height, int32_t *out_stride) {
  if (!sample || !out_bgra || !out_len) {
    return XHVP_ERR_FAIL;
  }
  GstCaps *caps = gst_sample_get_caps(sample);
  GstBuffer *buffer = gst_sample_get_buffer(sample);
  if (!caps || !buffer) {
    return XHVP_ERR_FAIL;
  }
  GstStructure *s = gst_caps_get_structure(caps, 0);
  int width = 0;
  int height = 0;
  if (!gst_structure_get_int(s, "width", &width) ||
      !gst_structure_get_int(s, "height", &height) || width <= 0 ||
      height <= 0) {
    return XHVP_ERR_FAIL;
  }

  GstMapInfo map;
  if (!gst_buffer_map(buffer, &map, GST_MAP_READ)) {
    return XHVP_ERR_FAIL;
  }

  const int dst_stride = width * 4;
  const uint32_t needed = (uint32_t)(dst_stride * height);
  int src_stride = dst_stride;
  GstVideoMeta *vmeta = gst_buffer_get_video_meta(buffer);
  if (vmeta && vmeta->stride[0] > 0) {
    src_stride = vmeta->stride[0];
  }

  uint8_t *dst = g_malloc(needed);
  if (src_stride == dst_stride && map.size >= needed) {
    memcpy(dst, map.data, needed);
  } else {
    const uint8_t *src = map.data;
    for (int row = 0; row < height; row++) {
      const gsize row_off = (gsize)row * (gsize)src_stride;
      const gsize copy_n = (gsize)dst_stride;
      if (row_off + copy_n > map.size) {
        memset(dst + (gsize)row * (gsize)dst_stride, 0, copy_n);
        continue;
      }
      memcpy(dst + (gsize)row * (gsize)dst_stride, src + row_off, copy_n);
    }
  }
  gst_buffer_unmap(buffer, &map);

  *out_bgra = dst;
  *out_len = needed;
  if (out_width) {
    *out_width = width;
  }
  if (out_height) {
    *out_height = height;
  }
  if (out_stride) {
    *out_stride = dst_stride;
  }
  return XHVP_ERR_OK;
}

static int32_t xhvp_thumb_run(const char *uri, int64_t position_ms,
                              int32_t max_width, uint8_t **out_bgra,
                              uint32_t *out_len, int32_t *out_width,
                              int32_t *out_height, int32_t *out_stride) {
  if (!uri || !*uri || !out_bgra || !out_len) {
    return XHVP_ERR_FAIL;
  }
  *out_bgra = NULL;
  *out_len = 0;

  if (max_width <= 0) {
    max_width = 320;
  }

  GstElement *pipeline = gst_element_factory_make("playbin3", "xhvp-thumb");
  if (!pipeline) {
    pipeline = gst_element_factory_make("playbin", "xhvp-thumb");
  }
  if (!pipeline) {
    return XHVP_ERR_FAIL;
  }

  GstElement *vsink = xhvp_thumb_make_video_sink(max_width);
  GstElement *asink = gst_element_factory_make("fakesink", "xhvp-thumb-audio");
  if (!vsink || !asink) {
    if (vsink) {
      gst_object_unref(vsink);
    }
    if (asink) {
      gst_object_unref(asink);
    }
    gst_object_unref(pipeline);
    return XHVP_ERR_FAIL;
  }
  g_object_set(asink, "sync", FALSE, NULL);

  g_object_set(pipeline, "uri", uri, "video-sink", vsink, "audio-sink", asink,
               "flags", GST_PLAY_FLAG_VIDEO, NULL);
  g_signal_connect(pipeline, "source-setup",
                   G_CALLBACK(xhvp_thumb_on_source_setup), NULL);
  g_signal_connect(pipeline, "element-setup",
                   G_CALLBACK(xhvp_thumb_on_element_setup), NULL);

  GstElement *appsink =
      gst_bin_get_by_name(GST_BIN(vsink), "xhvp-thumb-sink");
  if (!appsink) {
    gst_element_set_state(pipeline, GST_STATE_NULL);
    gst_object_unref(pipeline);
    return XHVP_ERR_FAIL;
  }

  GstStateChangeReturn ret = gst_element_set_state(pipeline, GST_STATE_PAUSED);
  if (ret == GST_STATE_CHANGE_FAILURE) {
    gst_object_unref(appsink);
    gst_element_set_state(pipeline, GST_STATE_NULL);
    gst_object_unref(pipeline);
    return XHVP_ERR_FAIL;
  }
  if (ret == GST_STATE_CHANGE_NO_PREROLL) {
    /* Live sources are not supported for thumbnails. */
    gst_object_unref(appsink);
    gst_element_set_state(pipeline, GST_STATE_NULL);
    gst_object_unref(pipeline);
    return XHVP_ERR_FAIL;
  }

  ret = gst_element_get_state(pipeline, NULL, NULL, 15 * GST_SECOND);
  if (ret == GST_STATE_CHANGE_FAILURE) {
    gst_object_unref(appsink);
    gst_element_set_state(pipeline, GST_STATE_NULL);
    gst_object_unref(pipeline);
    return XHVP_ERR_FAIL;
  }

  gint64 duration = GST_CLOCK_TIME_NONE;
  gst_element_query_duration(pipeline, GST_FORMAT_TIME, &duration);

  gint64 seek_pos;
  if (position_ms >= 0) {
    seek_pos = (gint64)position_ms * GST_MSECOND;
  } else if (GST_CLOCK_TIME_IS_VALID(duration) && duration > 0) {
    seek_pos = duration * 5 / 100;
  } else {
    seek_pos = 1 * GST_SECOND;
  }
  if (GST_CLOCK_TIME_IS_VALID(duration) && duration > 0 &&
      seek_pos >= duration) {
    seek_pos = duration > GST_SECOND ? duration - GST_SECOND : 0;
  }

  /* Seek failure is non-fatal: fall back to the preroll/first frame. */
  if (!gst_element_seek_simple(pipeline, GST_FORMAT_TIME,
                               GST_SEEK_FLAG_KEY_UNIT | GST_SEEK_FLAG_FLUSH,
                               seek_pos)) {
    /* Keep preroll sample as-is. */
  } else {
    (void)gst_element_get_state(pipeline, NULL, NULL, 10 * GST_SECOND);
  }

  GstSample *sample =
      gst_app_sink_try_pull_preroll(GST_APP_SINK(appsink), 10 * GST_SECOND);
  if (!sample) {
    sample =
        gst_app_sink_try_pull_sample(GST_APP_SINK(appsink), 2 * GST_SECOND);
  }

  int32_t rc = XHVP_ERR_FAIL;
  if (sample) {
    rc = xhvp_thumb_copy_sample(sample, out_bgra, out_len, out_width,
                                out_height, out_stride);
    gst_sample_unref(sample);
  }

  gst_object_unref(appsink);
  gst_element_set_state(pipeline, GST_STATE_NULL);
  gst_object_unref(pipeline);
  return rc;
}

static gpointer xhvp_thumb_thread_main(gpointer data) {
  XhvpThumbnailOp *op = data;
  op->result = xhvp_thumb_run(op->uri, op->position_ms, op->max_width,
                              op->out_bgra, op->out_len, op->out_width,
                              op->out_height, op->out_stride);
  return NULL;
}

int32_t xhvp_thumbnail_capture(const char *uri, int64_t position_ms,
                               int32_t max_width, uint8_t **out_bgra,
                               uint32_t *out_len, int32_t *out_width,
                               int32_t *out_height, int32_t *out_stride) {
  if (xhvp_init() != XHVP_ERR_OK) {
    return XHVP_ERR_FAIL;
  }
  if (!out_bgra || !out_len) {
    return XHVP_ERR_FAIL;
  }
  *out_bgra = NULL;
  *out_len = 0;

  /* Run on a dedicated thread so blocking preroll/seek/pull does not stall
   * the shared xhvp-gst main loop (active playback / other invoke_sync). */
  XhvpThumbnailOp op = {.uri = uri,
                        .position_ms = position_ms,
                        .max_width = max_width,
                        .out_bgra = out_bgra,
                        .out_len = out_len,
                        .out_width = out_width,
                        .out_height = out_height,
                        .out_stride = out_stride,
                        .result = XHVP_ERR_FAIL};
  GThread *thread =
      g_thread_new("xhvp-thumb", xhvp_thumb_thread_main, &op);
  if (!thread) {
    return XHVP_ERR_FAIL;
  }
  g_thread_join(thread);
  return op.result;
}

void xhvp_thumbnail_free(uint8_t *data) { g_free(data); }
