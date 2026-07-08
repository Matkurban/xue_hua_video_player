#ifndef XUE_HUA_VIDEO_TEXTURE_H_
#define XUE_HUA_VIDEO_TEXTURE_H_

#include <cstdint>
#include <map>
#include <memory>
#include <mutex>
#include <vector>

#include <flutter/texture_registrar.h>

namespace xue_hua_video {

/// Pulls BGRA frames from Rust and exposes them to Flutter as a pixel-buffer
/// external texture (RGBA, tight stride).
class VideoTexture {
 public:
  VideoTexture(int64_t player_id, flutter::TextureRegistrar* registrar);
  ~VideoTexture();

  VideoTexture(const VideoTexture&) = delete;
  VideoTexture& operator=(const VideoTexture&) = delete;

  int64_t texture_id() const { return texture_id_; }

  void OnFrameAvailable();

 private:
  const FlutterDesktopPixelBuffer* CopyPixelBuffer(size_t width, size_t height);

  int64_t player_id_;
  flutter::TextureRegistrar* registrar_;
  int64_t texture_id_ = -1;
  std::unique_ptr<flutter::TextureVariant> texture_variant_;

  std::mutex lock_;
  std::vector<uint8_t> staging_bgra_;
  std::vector<uint8_t> rgba_;
  FlutterDesktopPixelBuffer pixel_buffer_{};
  int32_t frame_width_ = 0;
  int32_t frame_height_ = 0;
};

/// Owns per-player [VideoTexture] instances for the plugin MethodChannel.
class VideoTextureRegistry {
 public:
  explicit VideoTextureRegistry(flutter::TextureRegistrar* registrar);

  int64_t Create(int64_t player_id);
  void Dispose(int64_t player_id);

 private:
  flutter::TextureRegistrar* registrar_;
  std::mutex lock_;
  std::map<int64_t, std::unique_ptr<VideoTexture>> textures_;
};

}  // namespace xue_hua_video

#endif  // XUE_HUA_VIDEO_TEXTURE_H_
