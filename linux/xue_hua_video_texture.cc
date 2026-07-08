#include "xue_hua_video_texture.h"

#include <flutter_linux/flutter_linux.h>

#include <cstring>
#include <mutex>
#include <vector>

G_DECLARE_FINAL_TYPE(XueHuaVideoTexture,
                     xue_hua_video_texture,
                     XUE_HUA,
                     VIDEO_TEXTURE,
                     FlPixelBufferTexture)

extern "C" {
void xhvp_texture_register(int64_t player_id,
                           void* ctx,
                           void (*on_frame)(void*));
void xhvp_texture_unregister(int64_t player_id);
bool xhvp_texture_frame_info(int64_t player_id,
                             int32_t* out_width,
                             int32_t* out_height,
                             int32_t* out_stride,
                             uint32_t* out_bytes);
bool xhvp_texture_copy_latest(int64_t player_id,
                              uint8_t* dst,
                              uint32_t dst_len,
                              int32_t* out_width,
                              int32_t* out_height,
                              int32_t* out_stride);
}

struct _XueHuaVideoTexture {
  FlPixelBufferTexture parent_instance;
  int64_t player_id;
  FlTextureRegistrar* registrar;
  std::mutex* lock;
  std::vector<uint8_t>* staging_bgra;
  std::vector<uint8_t>* rgba;
};

namespace {

void XhvpTextureOnFrame(void* ctx) {
  if (!ctx) {
    return;
  }
  auto* texture = static_cast<XueHuaVideoTexture*>(ctx);
  if (texture->registrar) {
    fl_texture_registrar_mark_texture_frame_available(texture->registrar,
                                                      FL_TEXTURE(texture));
  }
}

void BgraToRgbaTight(const uint8_t* src,
                     int32_t width,
                     int32_t height,
                     int32_t src_stride,
                     uint8_t* dst) {
  const int row_bytes = width * 4;
  for (int32_t y = 0; y < height; ++y) {
    const uint8_t* src_row = src + y * src_stride;
    uint8_t* dst_row = dst + y * row_bytes;
    for (int32_t x = 0; x < width; ++x) {
      dst_row[x * 4 + 0] = src_row[x * 4 + 2];
      dst_row[x * 4 + 1] = src_row[x * 4 + 1];
      dst_row[x * 4 + 2] = src_row[x * 4 + 0];
      dst_row[x * 4 + 3] = src_row[x * 4 + 3];
    }
  }
}

gboolean xue_hua_video_texture_copy_pixels(FlPixelBufferTexture* texture,
                                           const uint8_t** out_buffer,
                                           uint32_t* width,
                                           uint32_t* height,
                                           GError** error) {
  auto* self = XUE_HUA_VIDEO_TEXTURE(texture);
  std::lock_guard<std::mutex> guard(*self->lock);

  int32_t frame_width = 0;
  int32_t frame_height = 0;
  int32_t frame_stride = 0;
  uint32_t frame_bytes = 0;
  if (!xhvp_texture_frame_info(self->player_id, &frame_width, &frame_height,
                               &frame_stride, &frame_bytes) ||
      frame_width <= 0 || frame_height <= 0 || frame_bytes == 0) {
    if (error) {
      *error = nullptr;
    }
    return FALSE;
  }

  if (self->staging_bgra->size() < frame_bytes) {
    self->staging_bgra->resize(frame_bytes);
  }

  int32_t copied_w = 0;
  int32_t copied_h = 0;
  int32_t copied_stride = 0;
  if (!xhvp_texture_copy_latest(self->player_id, self->staging_bgra->data(),
                                frame_bytes, &copied_w, &copied_h,
                                &copied_stride) ||
      copied_w <= 0 || copied_h <= 0) {
    if (error) {
      *error = nullptr;
    }
    return FALSE;
  }

  const size_t rgba_bytes =
      static_cast<size_t>(copied_w) * static_cast<size_t>(copied_h) * 4;
  if (self->rgba->size() < rgba_bytes) {
    self->rgba->resize(rgba_bytes);
  }

  BgraToRgbaTight(self->staging_bgra->data(), copied_w, copied_h,
                  copied_stride, self->rgba->data());

  *out_buffer = self->rgba->data();
  *width = static_cast<uint32_t>(copied_w);
  *height = static_cast<uint32_t>(copied_h);
  if (error) {
    *error = nullptr;
  }
  return TRUE;
}

void xue_hua_video_texture_dispose(GObject* object) {
  auto* self = XUE_HUA_VIDEO_TEXTURE(object);
  xhvp_texture_unregister(self->player_id);
  delete self->lock;
  delete self->staging_bgra;
  delete self->rgba;
  self->lock = nullptr;
  self->staging_bgra = nullptr;
  self->rgba = nullptr;
  G_OBJECT_CLASS(xue_hua_video_texture_parent_class)->dispose(object);
}

void xue_hua_video_texture_class_init(XueHuaVideoTextureClass* klass) {
  G_OBJECT_CLASS(klass)->dispose = xue_hua_video_texture_dispose;
  FL_PIXEL_BUFFER_TEXTURE_CLASS(klass)->copy_pixels =
      xue_hua_video_texture_copy_pixels;
}

void xue_hua_video_texture_init(XueHuaVideoTexture* self) {
  self->player_id = 0;
  self->registrar = nullptr;
  self->lock = new std::mutex();
  self->staging_bgra = new std::vector<uint8_t>();
  self->rgba = new std::vector<uint8_t>();
}

}  // namespace

G_DEFINE_TYPE(XueHuaVideoTexture,
              xue_hua_video_texture,
              fl_pixel_buffer_texture_get_type())

XueHuaVideoTexture* xue_hua_video_texture_new(int64_t player_id,
                                              FlTextureRegistrar* registrar) {
  auto* texture = XUE_HUA_VIDEO_TEXTURE(
      g_object_new(xue_hua_video_texture_get_type(), nullptr));
  texture->player_id = player_id;
  texture->registrar = registrar;
  xhvp_texture_register(player_id, texture, XhvpTextureOnFrame);
  return texture;
}

void xue_hua_video_texture_dispose_instance(XueHuaVideoTexture* texture,
                                            FlTextureRegistrar* registrar) {
  if (!texture) {
    return;
  }
  fl_texture_registrar_unregister_texture(registrar, FL_TEXTURE(texture));
  g_object_unref(texture);
}
