use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::frb_generated::StreamSink;
use crate::player::GstPlayer;
// Re-exported so flutter_rust_bridge generates the matching Dart types.
pub use crate::player::{PlayerEvent, PlayerEventKind, PlayerState};

static PLAYERS: Lazy<Mutex<HashMap<i64, Arc<GstPlayer>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_ID: AtomicI64 = AtomicI64::new(1);

/// Identifiers returned when a player is created. `texture_id` is passed to the
/// Flutter `Texture` widget; `player_id` addresses all further control calls.
pub struct PlayerHandle {
    pub player_id: i64,
    pub texture_id: i64,
}

fn get_player(id: i64) -> Result<Arc<GstPlayer>> {
    PLAYERS
        .lock()
        .get(&id)
        .cloned()
        .ok_or_else(|| anyhow!("player {id} not found (already disposed?)"))
}

/// Creates a new player and its backing Flutter texture.
///
/// `engine_handle` must be obtained on the Dart side via
/// `EngineContext.instance.getEngineHandle()`.
pub fn create_player(engine_handle: i64) -> Result<PlayerHandle> {
    let player = GstPlayer::new(engine_handle)?;
    let texture_id = player.texture_id();
    let player_id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    PLAYERS.lock().insert(player_id, Arc::new(player));
    Ok(PlayerHandle { player_id, texture_id })
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

/// Loads a media URI (`file://...`, `http(s)://...`, `rtsp://...`) and prerolls.
pub fn player_set_source(player_id: i64, uri: String) -> Result<()> {
    get_player(player_id)?.set_uri(&uri)
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

/// Tears down the player, stops the pipeline and releases the texture.
pub fn dispose_player(player_id: i64) -> Result<()> {
    PLAYERS.lock().remove(&player_id);
    Ok(())
}
