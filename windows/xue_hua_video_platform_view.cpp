#include "xue_hua_video_platform_view.h"

extern "C" void player_set_video_overlay_window(int64_t player_id,
                                                int64_t window_handle);
extern "C" void player_sync_video_overlay_rectangle(int64_t player_id,
                                                    int32_t width,
                                                    int32_t height);

DesktopVideoOverlay::DesktopVideoOverlay(HWND parent) : parent_(parent) {}

DesktopVideoOverlay::~DesktopVideoOverlay() {
  for (auto& entry : overlays_) {
    player_set_video_overlay_window(entry.first, 0);
    if (entry.second) {
      ::DestroyWindow(entry.second);
    }
  }
  overlays_.clear();
}

HWND DesktopVideoOverlay::CreateOverlayHwnd() {
  if (!parent_) {
    return nullptr;
  }
  return ::CreateWindowExA(
      0, "STATIC", nullptr, WS_CHILD | WS_VISIBLE, 0, 0, 1, 1, parent_,
      nullptr, ::GetModuleHandle(nullptr), nullptr);
}

void DesktopVideoOverlay::BindOverlay(int64_t player_id, HWND hwnd) {
  if (!hwnd) {
    return;
  }
  player_set_video_overlay_window(
      player_id, reinterpret_cast<int64_t>(hwnd));
}

void DesktopVideoOverlay::Attach(int64_t player_id) {
  if (player_id == 0 || overlays_.count(player_id) != 0) {
    return;
  }
  HWND hwnd = CreateOverlayHwnd();
  if (!hwnd) {
    return;
  }
  overlays_[player_id] = hwnd;
  BindOverlay(player_id, hwnd);
}

void DesktopVideoOverlay::Detach(int64_t player_id) {
  auto it = overlays_.find(player_id);
  if (it == overlays_.end()) {
    return;
  }
  player_set_video_overlay_window(player_id, 0);
  if (it->second) {
    ::DestroyWindow(it->second);
  }
  overlays_.erase(it);
}

void DesktopVideoOverlay::SetBounds(int64_t player_id,
                                    double x,
                                    double y,
                                    double width,
                                    double height) {
  auto it = overlays_.find(player_id);
  if (it == overlays_.end() || !it->second || !parent_) {
    return;
  }
  POINT pt = {static_cast<LONG>(x), static_cast<LONG>(y)};
  ::ScreenToClient(parent_, &pt);
  const int w = static_cast<int>(width);
  const int h = static_cast<int>(height);
  ::MoveWindow(it->second, pt.x, pt.y, w > 0 ? w : 1, h > 0 ? h : 1, TRUE);
  BindOverlay(player_id, it->second);
  player_sync_video_overlay_rectangle(player_id, w > 0 ? w : 1, h > 0 ? h : 1);
}
