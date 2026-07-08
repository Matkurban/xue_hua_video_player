//! playbin3 多轨缓存与选择 / playbin3 multi-track cache and selection.
//!
//! 从 GStreamer `StreamCollection` 总线消息构建 [`TrackCache`]，将 GStreamer `stream-id`
//! 与 Dart [`MediaTrack`] 并行存储，并通过 `GST_EVENT_SELECT_STREAMS` 实现音轨/字幕切换。
//!
//! Builds [`TrackCache`] from GStreamer `StreamCollection` bus messages, storing parallel
//! GStreamer `stream-id` values with Dart [`MediaTrack`] entries, and switches tracks via
//! `GST_EVENT_SELECT_STREAMS`.

use std::sync::Arc;

use gstreamer as gst;
use gstreamer::prelude::*;
use parking_lot::Mutex;

use crate::player_events::{MediaTrack, TrackType};

/// 缓存的 playbin3 轨道及并行 GStreamer `stream-id`（用于选择）/ Cached playbin3 tracks plus parallel GStreamer `stream-id` values for selection.
#[derive(Debug, Clone, Default)]
pub struct TrackCache {
    tracks: Vec<MediaTrack>,
    stream_ids: Vec<String>,
}

impl TrackCache {
    /// 只读访问缓存轨道列表 / Read-only access to cached tracks.
    pub fn tracks(&self) -> &[MediaTrack] {
        &self.tracks
    }

    /// 清空缓存（`load` 新源时调用）/ Clears cache (called on new `load`).
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.stream_ids.clear();
    }

    /// 按轨道类型与逻辑 id 查找 GStreamer stream-id / Looks up GStreamer stream-id by track type and logical id.
    ///
    /// # 参数 / Parameters
    /// - `track_type` — 音轨/视频/字幕 / audio/video/subtitle
    /// - `id` — 逻辑轨道 id / logical track id
    ///
    /// # 返回值 / Returns
    /// - 匹配的 `stream-id` 或 `None` / matching stream-id or `None`
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 任意线程 / any thread
    ///
    /// # 平台 / Platform
    /// - 仅 playbin URI 管线 / playbin URI pipelines only
    pub fn stream_id_for(&self, track_type: TrackType, id: u32) -> Option<&str> {
        self.tracks
            .iter()
            .zip(self.stream_ids.iter())
            .find(|(t, _)| t.track_type == track_type && t.id == id)
            .map(|(_, sid)| sid.as_str())
    }
}

fn tag_language(tags: &gst::TagList) -> String {
    use gst::tags::LanguageCode;
    tags.get::<LanguageCode>()
        .map(|l| l.get().to_string())
        .unwrap_or_default()
}

fn tag_title(tags: &gst::TagList) -> String {
    use gst::tags::Title;
    tags.get::<Title>()
        .map(|t| t.get().to_string())
        .unwrap_or_default()
}

fn stream_type_to_track_type(stream_type: gst::StreamType) -> Option<TrackType> {
    if stream_type.contains(gst::StreamType::AUDIO) {
        Some(TrackType::Audio)
    } else if stream_type.contains(gst::StreamType::VIDEO) {
        Some(TrackType::Video)
    } else if stream_type.contains(gst::StreamType::TEXT) {
        Some(TrackType::Subtitle)
    } else {
        None
    }
}

/// 从 playbin3 [`gst::StreamCollection`] 构建 [`TrackCache`] / Builds [`TrackCache`] from playbin3 collection.
///
/// # 参数 / Parameters
/// - `collection` — GStreamer 流集合 / stream collection
///
/// # 返回值 / Returns
/// - 填充的 [`TrackCache`] / populated cache
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - 通常在总线消息处理中调用 / typically called from bus message handling
///
/// # 平台 / Platform
/// - playbin3 URI 管线 / playbin3 URI pipelines
pub fn tracks_from_collection(collection: &gst::StreamCollection) -> TrackCache {
    let mut cache = TrackCache::default();
    let mut audio_idx = 0u32;
    let mut video_idx = 0u32;
    let mut text_idx = 0u32;

    for stream in collection.iter() {
        let Some(track_type) = stream_type_to_track_type(stream.stream_type()) else {
            continue;
        };
        let stream_id = stream
            .stream_id()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if stream_id.is_empty() {
            continue;
        };

        let id = match track_type {
            TrackType::Audio => {
                let id = audio_idx;
                audio_idx += 1;
                id
            }
            TrackType::Video => {
                let id = video_idx;
                video_idx += 1;
                id
            }
            TrackType::Subtitle => {
                let id = text_idx;
                text_idx += 1;
                id
            }
        };
        let tags = stream.tags();
        let language = tags.as_ref().map(tag_language).unwrap_or_default();
        let label = tags
            .as_ref()
            .map(tag_title)
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| format!("{track_type:?} {id}"));

        cache.tracks.push(MediaTrack {
            id,
            track_type,
            language,
            label,
            selected: false,
        });
        cache.stream_ids.push(stream_id);
    }
    cache
}

/// 用总线 `StreamCollection` 消息替换缓存 / Replaces cache from bus `StreamCollection` message.
///
/// # 参数 / Parameters
/// - `collection` — 流集合 / stream collection
/// - `cache` — 共享缓存锁 / shared cache lock
///
/// # 返回值 / Returns
/// - 无 / None
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - 总线回调线程 / bus callback thread
///
/// # 平台 / Platform
/// - playbin3 / playbin3
pub fn update_cache_from_collection(
    collection: &gst::StreamCollection,
    cache: &Arc<Mutex<TrackCache>>,
) {
    *cache.lock() = tracks_from_collection(collection);
}

/// 用总线 `StreamsSelected` 消息更新 `selected` 标志 / Updates `selected` flags from `StreamsSelected` message.
///
/// # 参数 / Parameters
/// - `message` — 已选流消息 / streams selected message
/// - `cache` — 共享缓存 / shared cache
///
/// # 返回值 / Returns
/// - 无 / None
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - 总线回调线程 / bus callback thread
///
/// # 平台 / Platform
/// - playbin3 / playbin3
pub fn mark_selected_streams(
    message: &gst::message::StreamsSelected,
    cache: &Arc<Mutex<TrackCache>>,
) {
    let selected_ids: Vec<String> = message
        .streams()
        .filter_map(|stream| stream.stream_id().map(|id| id.to_string()))
        .collect();

    let mut guard = cache.lock();
    let stream_ids = guard.stream_ids.clone();
    for (track, stream_id) in guard.tracks.iter_mut().zip(stream_ids.iter()) {
        track.selected = selected_ids.iter().any(|id| id == stream_id);
    }
}

/// 返回缓存轨道快照（首条 stream-collection 消息前为空）/ Returns cached track snapshot (empty until first collection message).
///
/// # 参数 / Parameters
/// - `cache` — 共享缓存 / shared cache
///
/// # 返回值 / Returns
/// - [`MediaTrack`] 向量副本 / vector copy of tracks
///
/// # 错误 / Errors
/// - 无 / None
///
/// # 线程 / Threading
/// - 任意线程（短暂持锁）/ any thread, brief lock
///
/// # 平台 / Platform
/// - playbin3 / playbin3
pub fn read_cached_tracks(cache: &Arc<Mutex<TrackCache>>) -> Vec<MediaTrack> {
    cache.lock().tracks().to_vec()
}

fn selection_stream_ids(
    cache: &TrackCache,
    change_type: Option<(TrackType, u32)>,
    include_subtitles: bool,
) -> Vec<String> {
    let mut audio: Option<String> = None;
    let mut video: Option<String> = None;
    let mut subtitle: Option<String> = None;

    for (track, stream_id) in cache.tracks.iter().zip(cache.stream_ids.iter()) {
        if let Some((ty, id)) = change_type {
            if track.track_type == ty && track.id == id {
                match ty {
                    TrackType::Audio => audio = Some(stream_id.clone()),
                    TrackType::Video => video = Some(stream_id.clone()),
                    TrackType::Subtitle => subtitle = Some(stream_id.clone()),
                }
                continue;
            }
        }
        if track.track_type == TrackType::Subtitle && !include_subtitles {
            continue;
        }
        if track.selected {
            match track.track_type {
                TrackType::Audio if audio.is_none() => audio = Some(stream_id.clone()),
                TrackType::Video if video.is_none() => video = Some(stream_id.clone()),
                TrackType::Subtitle if subtitle.is_none() && include_subtitles => {
                    subtitle = Some(stream_id.clone());
                }
                _ => {}
            }
        }
    }

    if let Some((ty, id)) = change_type {
        if match ty {
            TrackType::Audio => audio.is_none(),
            TrackType::Video => video.is_none(),
            TrackType::Subtitle => subtitle.is_none(),
        } {
            if let Some(sid) = cache.stream_id_for(ty, id) {
                match ty {
                    TrackType::Audio => audio = Some(sid.to_string()),
                    TrackType::Video => video = Some(sid.to_string()),
                    TrackType::Subtitle => subtitle = Some(sid.to_string()),
                }
            }
        }
    }

    if video.is_none() {
        video = cache
            .tracks
            .iter()
            .zip(cache.stream_ids.iter())
            .find(|(t, _)| t.track_type == TrackType::Video)
            .map(|(_, sid)| sid.clone());
    }
    if audio.is_none() {
        audio = cache
            .tracks
            .iter()
            .zip(cache.stream_ids.iter())
            .find(|(t, _)| t.track_type == TrackType::Audio)
            .map(|(_, sid)| sid.clone());
    }

    [video, audio, subtitle].into_iter().flatten().collect()
}

/// 为单条轨道发送 `GST_EVENT_SELECT_STREAMS`（保留其他已选类型）/ Sends `GST_EVENT_SELECT_STREAMS` for one track.
///
/// # 参数 / Parameters
/// - `pipeline` — playbin 管线 / playbin pipeline
/// - `cache` — 轨道缓存 / track cache
/// - `track_type`、`track_id` — 目标轨道 / target track
///
/// # 返回值 / Returns
/// - 无 / None
///
/// # 错误 / Errors
/// - 事件被拒绝时记录 warn / logs warn if event rejected
///
/// # 线程 / Threading
/// - Gst 线程 / Gst thread
///
/// # 平台 / Platform
/// - playbin3 / playbin3
pub fn select_track_on_pipeline(
    pipeline: &gst::Pipeline,
    cache: &TrackCache,
    track_type: TrackType,
    track_id: u32,
) {
    let stream_ids = selection_stream_ids(cache, Some((track_type, track_id)), true);
    send_select_streams(pipeline, &stream_ids);
}

/// 禁用字幕：仅选择非 text 流 / Disables subtitles by selecting only non-text streams.
///
/// # 参数 / Parameters
/// - `pipeline` — playbin 管线 / playbin pipeline
/// - `cache` — 轨道缓存 / track cache
///
/// # 返回值 / Returns
/// - 无 / None
///
/// # 错误 / Errors
/// - 事件被拒绝时记录 warn / logs warn if rejected
///
/// # 线程 / Threading
/// - Gst 线程 / Gst thread
///
/// # 平台 / Platform
/// - playbin3 / playbin3
pub fn disable_subtitles_on_pipeline(pipeline: &gst::Pipeline, cache: &TrackCache) {
    let stream_ids = selection_stream_ids(cache, None, false);
    send_select_streams(pipeline, &stream_ids);
}

fn send_select_streams(pipeline: &gst::Pipeline, stream_ids: &[String]) {
    if stream_ids.is_empty() {
        return;
    }
    let refs: Vec<&str> = stream_ids.iter().map(String::as_str).collect();
    let event = gst::event::SelectStreams::new(refs.iter().copied());
    if !pipeline.send_event(event) {
        log::warn!("SelectStreams rejected: {stream_ids:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gst::StreamFlags;

    fn init_gst() {
        let _ = gst::init();
    }

    #[test]
    fn tracks_from_collection_maps_types_and_ids() {
        init_gst();

        let audio = gst::Stream::new(
            Some("audio_0"),
            None,
            gst::StreamType::AUDIO,
            StreamFlags::SELECT,
        );
        let video = gst::Stream::new(
            Some("video_0"),
            None,
            gst::StreamType::VIDEO,
            StreamFlags::SELECT,
        );
        let collection = gst::StreamCollection::builder(None)
            .stream(audio)
            .stream(video)
            .build();

        let cache = tracks_from_collection(&collection);
        assert_eq!(cache.tracks.len(), 2);
        assert_eq!(cache.stream_ids, vec!["audio_0", "video_0"]);

        let audio_track = cache
            .tracks
            .iter()
            .find(|t| t.track_type == TrackType::Audio)
            .expect("audio");
        assert_eq!(audio_track.id, 0);
        assert_eq!(audio_track.label, "Audio 0");

        let video_track = cache
            .tracks
            .iter()
            .find(|t| t.track_type == TrackType::Video)
            .expect("video");
        assert_eq!(video_track.id, 0);
    }

    #[test]
    fn mark_selected_streams_updates_flags() {
        init_gst();

        let audio = gst::Stream::new(
            Some("audio_0"),
            None,
            gst::StreamType::AUDIO,
            StreamFlags::SELECT,
        );
        let collection = gst::StreamCollection::builder(None)
            .stream(audio.clone())
            .build();
        let cache = Arc::new(Mutex::new(tracks_from_collection(&collection)));

        let msg = gst::message::StreamsSelected::builder(&collection)
            .streams([&audio])
            .build();
        if let gst::MessageView::StreamsSelected(selected) = msg.view() {
            mark_selected_streams(&selected, &cache);
        }

        assert!(cache.lock().tracks[0].selected);
    }

    #[test]
    fn selection_stream_ids_excludes_subtitles_when_disabled() {
        init_gst();

        let mut cache = TrackCache::default();
        cache.tracks.push(MediaTrack {
            id: 0,
            track_type: TrackType::Audio,
            language: String::new(),
            label: "a".into(),
            selected: true,
        });
        cache.stream_ids.push("audio_0".into());
        cache.tracks.push(MediaTrack {
            id: 0,
            track_type: TrackType::Subtitle,
            language: String::new(),
            label: "s".into(),
            selected: true,
        });
        cache.stream_ids.push("text_0".into());

        let ids = selection_stream_ids(&cache, None, false);
        assert_eq!(ids, vec!["audio_0"]);
    }
}
