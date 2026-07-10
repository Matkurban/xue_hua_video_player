#include "xhvp_player.h"

/*
 * Dart resolves xhvp_* via DynamicLibrary.process() / dlsym on Apple.
 * Release builds dead-strip unreferenced globals; Swift only calls
 * xhvp_texture_*. Holding function pointers here (and calling this from
 * plugin register) keeps the ABI in the final binary.
 */
void xhvp_ffi_retain_symbols(void) {
  static void *const keep[] = {
      (void *)xhvp_version,
      (void *)xhvp_init,
      (void *)xhvp_shutdown,
      (void *)xhvp_player_create,
      (void *)xhvp_player_dispose,
      (void *)xhvp_player_set_event_callback,
      (void *)xhvp_player_load_uri,
      (void *)xhvp_player_load_asset,
      (void *)xhvp_player_play,
      (void *)xhvp_player_pause,
      (void *)xhvp_player_stop,
      (void *)xhvp_player_seek,
      (void *)xhvp_player_set_volume,
      (void *)xhvp_player_set_mute,
      (void *)xhvp_player_set_speed,
      (void *)xhvp_player_set_looping,
      (void *)xhvp_player_get_capabilities,
      (void *)xhvp_player_get_track_count,
      (void *)xhvp_player_get_track,
      (void *)xhvp_player_select_track,
      (void *)xhvp_player_set_video_rotation,
      (void *)xhvp_player_set_aspect_ratio_mode,
      (void *)xhvp_player_notify_android_surface,
      (void *)xhvp_player_clear_android_surface,
      (void *)xhvp_texture_register,
      (void *)xhvp_texture_unregister,
      (void *)xhvp_texture_frame_info,
      (void *)xhvp_texture_copy_latest,
  };
  /* Volatile read so the compiler cannot elide the table. */
  (void)*(volatile void *const *)&keep[0];
}
