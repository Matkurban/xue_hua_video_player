use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::frb_generated::StreamSink;
use crate::playback::PlaybackEngine;
pub use crate::player_events::{
    AspectRatioMode, MediaSourceDto, MediaTrack, PlayerEvent, PlayerEventKind, PlayerState,
    TrackType, VideoMetadata, VideoOrientationConfig,
};

static PLAYERS: Lazy<Mutex<HashMap<i64, Arc<PlaybackEngine>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static PENDING_OVERLAYS: Lazy<Mutex<HashMap<i64, i64>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_ID: AtomicI64 = AtomicI64::new(1);

/// Identifiers returned when a player is created. `player_id` addresses all
/// control calls; bind a Platform View with the same id via `creationParams`.
pub struct PlayerHandle {
    pub player_id: i64,
}

fn get_player(id: i64) -> Result<Arc<PlaybackEngine>> {
    PLAYERS
        .lock()
        .get(&id)
        .cloned()
        .ok_or_else(|| anyhow!("player {id} not found (already disposed?)"))
}

/// Creates a new GStreamer-backed player.
pub fn create_player() -> Result<PlayerHandle> {
    #[cfg(target_os = "android")]
    crate::diag::ensure_android_diagnostics_initialized();

    let player = PlaybackEngine::new()?;
    let player_id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    PLAYERS.lock().insert(player_id, Arc::new(player));
    if let Some(handle) = PENDING_OVERLAYS.lock().remove(&player_id) {
        #[cfg(target_os = "macos")]
        get_player(player_id)?.cache_macos_overlay_handle(handle);
        #[cfg(target_os = "android")]
        get_player(player_id)?.notify_android_surface(handle, 0, 0)?;
        #[cfg(not(any(target_os = "macos", target_os = "android")))]
        get_player(player_id)?.set_video_overlay_window(handle)?;
    }
    Ok(PlayerHandle { player_id })
}

/// macOS: synchronously records the NSView handle for bus sync / rebind.
#[cfg(target_os = "macos")]
pub fn cache_macos_overlay_handle(player_id: i64, view_ptr: i64) -> Result<()> {
    match get_player(player_id) {
        Ok(player) => {
            PENDING_OVERLAYS.lock().remove(&player_id);
            player.cache_macos_overlay_handle(view_ptr);
            Ok(())
        }
        Err(_) if view_ptr != 0 => {
            PENDING_OVERLAYS.lock().insert(player_id, view_ptr);
            Ok(())
        }
        Err(_) => {
            PENDING_OVERLAYS.lock().remove(&player_id);
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn cache_macos_overlay_handle(_player_id: i64, _view_ptr: i64) -> Result<()> {
    Ok(())
}

/// macOS: applies the cached NSView handle to the GStreamer sink (main thread).
#[cfg(target_os = "macos")]
pub fn apply_macos_overlay_gstreamer(player_id: i64, width: i32, height: i32) -> Result<()> {
    get_player(player_id)?.apply_macos_overlay_gstreamer(width, height)
}

#[cfg(not(target_os = "macos"))]
pub fn apply_macos_overlay_gstreamer(
    _player_id: i64,
    _width: i32,
    _height: i32,
) -> Result<()> {
    Ok(())
}

/// macOS: records the target `NSView` handle (apply via Swift main-thread dispatch).
#[cfg(target_os = "macos")]
pub fn sync_macos_video_layer(
    player_id: i64,
    view_ptr: i64,
    _width: i32,
    _height: i32,
) -> Result<()> {
    cache_macos_overlay_handle(player_id, view_ptr)
}

#[cfg(not(target_os = "macos"))]
pub fn sync_macos_video_layer(
    _player_id: i64,
    _view_ptr: i64,
    _width: i32,
    _height: i32,
) -> Result<()> {
    Ok(())
}

/// Binds a native window/surface handle to the player's VideoOverlay sink.
pub fn set_video_overlay_window(player_id: i64, window_handle: i64) -> Result<()> {
    match get_player(player_id) {
        Ok(player) => {
            PENDING_OVERLAYS.lock().remove(&player_id);
            player.set_video_overlay_window(window_handle)
        }
        Err(_) if window_handle != 0 => {
            PENDING_OVERLAYS.lock().insert(player_id, window_handle);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Android: caches `ANativeWindow` on the JNI thread and applies VideoOverlay on
/// `xhvp-gst` without blocking the Android main thread.
#[cfg(target_os = "android")]
pub fn notify_android_surface(
    player_id: i64,
    handle: i64,
    width: i32,
    height: i32,
) -> Result<()> {
    match get_player(player_id) {
        Ok(player) => {
            PENDING_OVERLAYS.lock().remove(&player_id);
            player.notify_android_surface(handle, width, height)
        }
        Err(_) if handle != 0 => {
            PENDING_OVERLAYS.lock().insert(player_id, handle);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[cfg(not(target_os = "android"))]
pub fn notify_android_surface(
    _player_id: i64,
    _handle: i64,
    _width: i32,
    _height: i32,
) -> Result<()> {
    Ok(())
}

/// Subscribes to the player's event stream (state, position, duration, size,
/// buffering, EOS, errors). Should be called once right after `create_player`.
pub fn player_event_stream(player_id: i64, sink: StreamSink<PlayerEvent>) -> Result<()> {
    let player = get_player(player_id)?;
    player.set_emitter(Arc::new(move |event| {
        let _ = sink.add(event);
    }));
    Ok(())
}

/// Loads media from a unified source descriptor (URI or Flutter asset).
pub fn player_load_source(player_id: i64, source: MediaSourceDto) -> Result<()> {
    get_player(player_id)?.load(source.into())
}

/// Loads a media URI (`file://...`, `http(s)://...`, `rtsp://...`) and prerolls.
pub fn player_set_source(player_id: i64, uri: String) -> Result<()> {
    get_player(player_id)?.set_uri(&uri)
}

/// Loads a Flutter asset key via AppSrc (no Dart-side temp file copy).
pub fn player_set_asset_source(player_id: i64, asset_key: String) -> Result<()> {
    get_player(player_id)?.set_asset_source(&asset_key)
}

pub fn player_play(player_id: i64) -> Result<()> {
    get_player(player_id)?.play()
}

pub fn player_pause(player_id: i64) -> Result<()> {
    get_player(player_id)?.pause()
}

pub fn player_stop(player_id: i64) -> Result<()> {
    get_player(player_id)?.stop()
}

pub fn player_seek(player_id: i64, position_ms: i64) -> Result<()> {
    get_player(player_id)?.seek(position_ms)
}

pub fn player_set_volume(player_id: i64, volume: f64) -> Result<()> {
    get_player(player_id)?.set_volume(volume);
    Ok(())
}

pub fn player_set_mute(player_id: i64, mute: bool) -> Result<()> {
    get_player(player_id)?.set_mute(mute);
    Ok(())
}

pub fn player_set_speed(player_id: i64, speed: f64) -> Result<()> {
    get_player(player_id)?.set_speed(speed)
}

pub fn player_set_looping(player_id: i64, looping: bool) -> Result<()> {
    get_player(player_id)?.set_looping(looping);
    Ok(())
}

pub fn player_position(player_id: i64) -> Result<i64> {
    Ok(get_player(player_id)?.position_ms())
}

pub fn player_duration(player_id: i64) -> Result<i64> {
    Ok(get_player(player_id)?.duration_ms())
}

pub fn player_is_seekable(player_id: i64) -> Result<bool> {
    Ok(get_player(player_id)?.is_seekable())
}

pub fn player_get_tracks(player_id: i64) -> Result<Vec<MediaTrack>> {
    Ok(get_player(player_id)?.tracks())
}

pub fn player_select_track(
    player_id: i64,
    track_id: u32,
    track_type: TrackType,
    enable: bool,
) -> Result<()> {
    get_player(player_id)?.select_track(track_id, track_type, enable)
}

pub fn player_get_video_metadata(player_id: i64) -> Result<VideoMetadata> {
    let meta = get_player(player_id)?.video_metadata();
    Ok(meta)
}

pub fn player_set_video_orientation(
    player_id: i64,
    config: VideoOrientationConfig,
) -> Result<()> {
    get_player(player_id)?.set_video_orientation(config.into())
}

pub fn player_set_aspect_ratio_mode(player_id: i64, mode: AspectRatioMode) -> Result<()> {
    get_player(player_id)?.set_aspect_ratio_mode(mode.into())
}

/// Tears down the player and stops the pipeline.
pub fn dispose_player(player_id: i64) -> Result<()> {
    PENDING_OVERLAYS.lock().remove(&player_id);
    PLAYERS.lock().remove(&player_id);
    Ok(())
}
