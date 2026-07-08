pub mod api;
mod frb_generated;
mod gst;
mod media;
mod platform;
mod playback;

pub(crate) mod diag;

// FRB-generated code references `crate::player_events`; keep alias at the seam.
pub(crate) use api::types as player_events;
