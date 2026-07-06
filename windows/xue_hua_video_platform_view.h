#ifndef XUE_HUA_VIDEO_PLAYER_WINDOWS_XUE_HUA_VIDEO_PLATFORM_VIEW_H_
#define XUE_HUA_VIDEO_PLAYER_WINDOWS_XUE_HUA_VIDEO_PLATFORM_VIEW_H_

#include <cstdint>
#include <map>

#include <windows.h>

// Manages child HWND overlays for GStreamer d3d11videosink VideoOverlay binding.
// Used via MethodChannel because Flutter Windows PlatformView API is not yet
// exposed in the public embedding headers.
class DesktopVideoOverlay {
 public:
  explicit DesktopVideoOverlay(HWND parent);
  ~DesktopVideoOverlay();

  void Attach(int64_t player_id);
  void Detach(int64_t player_id);
  void SetBounds(int64_t player_id,
                 double x,
                 double y,
                 double width,
                 double height);

 private:
  HWND parent_;
  std::map<int64_t, HWND> overlays_;

  HWND CreateOverlayHwnd();
  void BindOverlay(int64_t player_id, HWND hwnd);
};

#endif  // XUE_HUA_VIDEO_PLAYER_WINDOWS_XUE_HUA_VIDEO_PLATFORM_VIEW_H_
