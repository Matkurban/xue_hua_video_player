#include "xue_hua_video_texture.h"

#include <algorithm>
#include <cstring>

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

namespace {

void XhvpTextureOnFrame(void* ctx) {
  if (!ctx) {
    return;
  }
  static_cast<xue_hua_video::VideoTexture*>(ctx)->OnFrameAvailable();
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

}  // namespace

namespace xue_hua_video {

VideoTexture::VideoTexture(int64_t player_id,
                           flutter::TextureRegistrar* registrar)
    : player_id_(player_id), registrar_(registrar) {
  texture_variant_ = std::make_unique<flutter::TextureVariant>(
      flutter::PixelBufferTexture(
          [this](size_t width, size_t height) {
            return CopyPixelBuffer(width, height);
          }));
  texture_id_ = registrar_->RegisterTexture(texture_variant_.get());
  xhvp_texture_register(player_id_, this, XhvpTextureOnFrame);
}

VideoTexture::~VideoTexture() {
  xhvp_texture_unregister(player_id_);
  if (registrar_ && texture_id_ >= 0) {
    registrar_->UnregisterTexture(texture_id_, []() {});
  }
}

void VideoTexture::OnFrameAvailable() {
  if (registrar_ && texture_id_ >= 0) {
    registrar_->MarkTextureFrameAvailable(texture_id_);
  }
}

const FlutterDesktopPixelBuffer* VideoTexture::CopyPixelBuffer(size_t width,
                                                               size_t height) {
  (void)width;
  (void)height;

  std::lock_guard<std::mutex> guard(lock_);

  int32_t frame_width = 0;
  int32_t frame_height = 0;
  int32_t frame_stride = 0;
  uint32_t frame_bytes = 0;
  if (!xhvp_texture_frame_info(player_id_, &frame_width, &frame_height,
                               &frame_stride, &frame_bytes) ||
      frame_width <= 0 || frame_height <= 0 || frame_bytes == 0) {
    return nullptr;
  }

  if (staging_bgra_.size() < frame_bytes) {
    staging_bgra_.resize(frame_bytes);
  }

  int32_t copied_w = 0;
  int32_t copied_h = 0;
  int32_t copied_stride = 0;
  if (!xhvp_texture_copy_latest(player_id_, staging_bgra_.data(), frame_bytes,
                                &copied_w, &copied_h, &copied_stride) ||
      copied_w <= 0 || copied_h <= 0) {
    return nullptr;
  }

  const size_t rgba_bytes =
      static_cast<size_t>(copied_w) * static_cast<size_t>(copied_h) * 4;
  if (rgba_.size() < rgba_bytes) {
    rgba_.resize(rgba_bytes);
  }

  BgraToRgbaTight(staging_bgra_.data(), copied_w, copied_h, copied_stride,
                  rgba_.data());

  frame_width_ = copied_w;
  frame_height_ = copied_h;
  pixel_buffer_.buffer = rgba_.data();
  pixel_buffer_.width = static_cast<size_t>(frame_width_);
  pixel_buffer_.height = static_cast<size_t>(frame_height_);
  pixel_buffer_.release_callback = nullptr;
  pixel_buffer_.release_context = nullptr;
  return &pixel_buffer_;
}

VideoTextureRegistry::VideoTextureRegistry(
    flutter::TextureRegistrar* registrar)
    : registrar_(registrar) {}

int64_t VideoTextureRegistry::Create(int64_t player_id) {
  if (player_id == 0) {
    return -1;
  }
  std::lock_guard<std::mutex> guard(lock_);
  auto it = textures_.find(player_id);
  if (it != textures_.end()) {
    return it->second->texture_id();
  }
  auto texture = std::make_unique<VideoTexture>(player_id, registrar_);
  const int64_t texture_id = texture->texture_id();
  textures_.emplace(player_id, std::move(texture));
  return texture_id;
}

void VideoTextureRegistry::Dispose(int64_t player_id) {
  std::lock_guard<std::mutex> guard(lock_);
  textures_.erase(player_id);
}

}  // namespace xue_hua_video
