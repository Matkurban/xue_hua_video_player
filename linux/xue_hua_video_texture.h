#ifndef XUE_HUA_VIDEO_TEXTURE_H_
#define XUE_HUA_VIDEO_TEXTURE_H_

#include <glib-object.h>

G_BEGIN_DECLS

typedef struct _FlTextureRegistrar FlTextureRegistrar;
typedef struct _XueHuaVideoTexture XueHuaVideoTexture;

XueHuaVideoTexture* xue_hua_video_texture_new(int64_t player_id,
                                              FlTextureRegistrar* registrar);

void xue_hua_video_texture_dispose_instance(XueHuaVideoTexture* texture,
                                            FlTextureRegistrar* registrar);

G_END_DECLS

#endif  // XUE_HUA_VIDEO_TEXTURE_H_
