#include "xhvp_internal.h"

#include <gst/video/video.h>
#include <string.h>

void xhvp_frame_init(XhvpPlayer *p) {
  memset(p->frames, 0, sizeof(p->frames));
  p->latest_frame = 0;
}

void xhvp_frame_clear(XhvpPlayer *p) {
  pthread_mutex_lock(&p->frame_mu);
  for (int i = 0; i < 2; i++) {
    g_free(p->frames[i].data);
    p->frames[i].data = NULL;
    p->frames[i].capacity = 0;
    p->frames[i].size = 0;
    p->frames[i].valid = false;
  }
  p->latest_frame = 0;
  pthread_mutex_unlock(&p->frame_mu);
}

GstFlowReturn xhvp_frame_on_new_sample(GstAppSink *sink, gpointer user_data) {
  XhvpPlayer *p = user_data;
  if (!p || !p->in_use || !p->pipeline) {
    return GST_FLOW_OK;
  }
  GstSample *sample = gst_app_sink_pull_sample(sink);
  if (!sample) {
    return GST_FLOW_ERROR;
  }

  GstCaps *caps = gst_sample_get_caps(sample);
  GstBuffer *buffer = gst_sample_get_buffer(sample);
  if (!caps || !buffer) {
    gst_sample_unref(sample);
    return GST_FLOW_ERROR;
  }

  GstStructure *s = gst_caps_get_structure(caps, 0);
  int width = 0;
  int height = 0;
  gst_structure_get_int(s, "width", &width);
  gst_structure_get_int(s, "height", &height);
  if (width <= 0 || height <= 0) {
    gst_sample_unref(sample);
    return GST_FLOW_OK;
  }

  GstMapInfo map;
  if (!gst_buffer_map(buffer, &map, GST_MAP_READ)) {
    gst_sample_unref(sample);
    return GST_FLOW_ERROR;
  }

  const int dst_stride = width * 4;
  const uint32_t needed = (uint32_t)(dst_stride * height);
  int src_stride = dst_stride;
  GstVideoMeta *vmeta = gst_buffer_get_video_meta(buffer);
  if (vmeta && vmeta->stride[0] > 0) {
    src_stride = vmeta->stride[0];
  }

  pthread_mutex_lock(&p->frame_mu);
  const int write_idx = 1 - p->latest_frame;
  XhvpFrameBuffer *fb = &p->frames[write_idx];
  if (fb->capacity < needed) {
    fb->data = g_realloc(fb->data, needed);
    fb->capacity = needed;
  }
  if (src_stride == dst_stride && map.size >= needed) {
    memcpy(fb->data, map.data, needed);
  } else {
    const uint8_t *src = map.data;
    for (int row = 0; row < height; row++) {
      const gsize row_off = (gsize)row * (gsize)src_stride;
      const gsize copy_n = (gsize)dst_stride;
      if (row_off + copy_n > map.size) {
        memset(fb->data + (gsize)row * (gsize)dst_stride, 0, copy_n);
        continue;
      }
      memcpy(fb->data + (gsize)row * (gsize)dst_stride, src + row_off, copy_n);
    }
  }
  fb->size = needed;
  fb->width = width;
  fb->height = height;
  fb->stride = dst_stride;
  fb->valid = true;
  p->latest_frame = write_idx;

  if (p->width != width || p->height != height) {
    p->width = width;
    p->height = height;
    /* Always track post-transform size as DAR (videoflip swaps axes). */
    p->dar_n = width;
    p->dar_d = height;
    if (p->par_n == 0) {
      p->par_n = 1;
      p->par_d = 1;
    }
    xhvp_player_emit(p, XHVP_EVENT_VIDEO_SIZE, "");
    xhvp_player_emit(p, XHVP_EVENT_METADATA_CHANGED, "");
  }

  XhvpFrameReadyFn cb = p->frame_cb;
  void *ctx = p->frame_ctx;
  pthread_mutex_unlock(&p->frame_mu);

  gst_buffer_unmap(buffer, &map);
  gst_sample_unref(sample);

  if (cb) {
    cb(ctx);
  }
  return GST_FLOW_OK;
}

bool xhvp_frame_info(XhvpPlayer *p, int32_t *w, int32_t *h, int32_t *stride,
                     uint32_t *bytes) {
  pthread_mutex_lock(&p->frame_mu);
  XhvpFrameBuffer *fb = &p->frames[p->latest_frame];
  if (!fb->valid) {
    pthread_mutex_unlock(&p->frame_mu);
    return false;
  }
  if (w) {
    *w = fb->width;
  }
  if (h) {
    *h = fb->height;
  }
  if (stride) {
    *stride = fb->stride;
  }
  if (bytes) {
    *bytes = fb->size;
  }
  pthread_mutex_unlock(&p->frame_mu);
  return true;
}

bool xhvp_frame_copy(XhvpPlayer *p, uint8_t *dst, uint32_t dst_len, int32_t *w,
                     int32_t *h, int32_t *stride) {
  if (!dst) {
    return false;
  }
  pthread_mutex_lock(&p->frame_mu);
  XhvpFrameBuffer *fb = &p->frames[p->latest_frame];
  if (!fb->valid || dst_len < fb->size) {
    pthread_mutex_unlock(&p->frame_mu);
    return false;
  }
  memcpy(dst, fb->data, fb->size);
  if (w) {
    *w = fb->width;
  }
  if (h) {
    *h = fb->height;
  }
  if (stride) {
    *stride = fb->stride;
  }
  pthread_mutex_unlock(&p->frame_mu);
  return true;
}
