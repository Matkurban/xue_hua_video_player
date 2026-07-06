use gstreamer as gst;
use gstreamer::prelude::*;

use crate::player_events::{MediaTrack, TrackType};

fn read_playbin_tracks(
    playbin: &gst::Element,
    count_prop: &str,
    current_prop: &str,
    track_type: TrackType,
) -> Vec<MediaTrack> {
    let n: u32 = playbin.property(count_prop);
    let current: i32 = playbin.property(current_prop);
    let mut out = Vec::new();
    for i in 0..n {
        out.push(MediaTrack {
            id: i,
            track_type,
            language: String::new(),
            label: format!("{track_type:?} {i}"),
            selected: current == i as i32,
        });
    }
    out
}

/// Enumerates audio, video, and subtitle tracks exposed by playbin.
pub fn collect_playbin_tracks(playbin: &gst::Element) -> Vec<MediaTrack> {
    let mut tracks = Vec::new();
    tracks.extend(read_playbin_tracks(
        playbin,
        "n-audio",
        "current-audio",
        TrackType::Audio,
    ));
    tracks.extend(read_playbin_tracks(
        playbin,
        "n-video",
        "current-video",
        TrackType::Video,
    ));
    tracks.extend(read_playbin_tracks(
        playbin,
        "n-text",
        "current-text",
        TrackType::Subtitle,
    ));
    tracks
}

/// Selects an audio or subtitle track on playbin (`current-audio` / `current-text`).
pub fn select_playbin_track(playbin: &gst::Element, track: &MediaTrack) -> bool {
    match track.track_type {
        TrackType::Audio => {
            playbin.set_property("current-audio", track.id as i32);
            true
        }
        TrackType::Subtitle => {
            playbin.set_property("current-text", track.id as i32);
            true
        }
        TrackType::Video => {
            playbin.set_property("current-video", track.id as i32);
            true
        }
    }
}

/// Disables subtitle rendering on playbin.
pub fn disable_subtitles(playbin: &gst::Element) {
    playbin.set_property("current-text", -1i32);
}
