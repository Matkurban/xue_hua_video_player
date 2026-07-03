use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gstreamer_video::prelude::VideoFrameExt;
use irondash_texture::{BoxedPixelData, SendableTexture, Texture};
use irondash_run_loop::RunLoop;
use parking_lot::Mutex;

use crate::video_texture::{FrameBuffer, FrameProvider};

/// High-level playback state reported to Dart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Idle,
    Ready,
    Buffering,
    Playing,
    Paused,
    Stopped,
    Completed,
    Error,
}

/// Discriminates which fields of [`PlayerEvent`] are meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerEventKind {
    DurationChanged,
    PositionChanged,
    VideoSize,
    StateChanged,
    Buffering,
    Eos,
    Error,
}

/// A flat event struct pushed to Dart over a broadcast stream.
///
/// Modeled as a struct (rather than a data-carrying enum) so the generated Dart
/// bindings stay dependency-free (no `freezed`). Only the fields relevant to
/// `kind` are populated; others hold defaults.
#[derive(Debug, Clone)]
pub struct PlayerEvent {
    pub kind: PlayerEventKind,
    /// Milliseconds (for `PositionChanged`).
    pub position_ms: i64,
    /// Milliseconds (for `DurationChanged`).
    pub duration_ms: i64,
    /// Pixels (for `VideoSize`).
    pub width: i32,
    /// Pixels (for `VideoSize`).
    pub height: i32,
    /// 0-100 (for `Buffering`).
    pub buffering_percent: i32,
    /// New state (for `StateChanged`).
    pub state: PlayerState,
    /// Human-readable message (for `Error`).
    pub message: String,
}

impl PlayerEvent {
    fn base(kind: PlayerEventKind) -> Self {
        Self {
            kind,
            position_ms: 0,
            duration_ms: 0,
            width: 0,
            height: 0,
            buffering_percent: 0,
            state: PlayerState::Idle,
            message: String::new(),
        }
    }

    fn duration(duration_ms: i64) -> Self {
        Self { duration_ms, ..Self::base(PlayerEventKind::DurationChanged) }
    }

    fn position(position_ms: i64) -> Self {
        Self { position_ms, ..Self::base(PlayerEventKind::PositionChanged) }
    }

    fn video_size(width: i32, height: i32) -> Self {
        Self { width, height, ..Self::base(PlayerEventKind::VideoSize) }
    }

    fn state(state: PlayerState) -> Self {
        Self { state, ..Self::base(PlayerEventKind::StateChanged) }
    }

    fn buffering(buffering_percent: i32) -> Self {
        Self { buffering_percent, ..Self::base(PlayerEventKind::Buffering) }
    }

    fn eos() -> Self {
        Self::base(PlayerEventKind::Eos)
    }

    fn error(message: String) -> Self {
        Self { message, ..Self::base(PlayerEventKind::Error) }
    }
}

type Emitter = Arc<dyn Fn(PlayerEvent) + Send + Sync>;

// On Android, GStreamer and all of its plugins are compiled statically into a
// single `libgstreamer_android.so` (built via ndk-build; see
// `android/gstreamer_build`). Static plugins are not auto-discovered by
// scanning the filesystem, so they must be registered explicitly. This symbol
// is generated into that library and registers every bundled plugin.
#[cfg(target_os = "android")]
extern "C" {
    fn gst_init_static_plugins();
}

/// Ensures `gst::init()` runs exactly once for the process.
pub fn ensure_gst_init() -> Result<()> {
    use std::sync::Once;
    static INIT: Once = Once::new();
    static mut RESULT: Option<Result<()>> = None;
    // SAFETY: guarded by Once, only written inside call_once.
    unsafe {
        INIT.call_once(|| {
            RESULT = Some((|| {
                gst::init().map_err(|e| anyhow!("gst::init failed: {e}"))?;
                // Register the statically-linked plugins on Android.
                #[cfg(target_os = "android")]
                gst_init_static_plugins();
                Ok(())
            })());
        });
        match &*std::ptr::addr_of!(RESULT) {
            Some(Ok(())) => Ok(()),
            Some(Err(e)) => Err(anyhow!("{e}")),
            None => Err(anyhow!("gst init state missing")),
        }
    }
}

/// A single GStreamer `playbin3`-backed video player rendering into a Flutter
/// external texture.
pub struct GstPlayer {
    pipeline: gst::Pipeline,
    texture_id: i64,
    frame_buffer: Arc<FrameBuffer>,
    emitter: Arc<Mutex<Option<Emitter>>>,
    rate: Arc<Mutex<f64>>,
    looping: Arc<AtomicBool>,
    desired_playing: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    bus_thread: Mutex<Option<JoinHandle<()>>>,
    // Kept alive for the lifetime of the player; frames are pushed through it.
    _sendable: Arc<SendableTexture<BoxedPixelData>>,
}

impl GstPlayer {
    /// Creates a player and its Flutter texture. Must be able to reach the
    /// engine's platform thread (via irondash run loop) to register the texture.
    pub fn new(engine_handle: i64) -> Result<Self> {
        ensure_gst_init()?;

        let frame_buffer = FrameBuffer::new();
        let emitter: Arc<Mutex<Option<Emitter>>> = Arc::new(Mutex::new(None));

        // Create the texture on the platform (main) thread.
        let (texture_id, sendable) = {
            let frame_buffer = frame_buffer.clone();
            let sender = RunLoop::sender_for_main_thread()
                .map_err(|e| anyhow!("cannot reach main thread run loop: {e:?}"))?;
            sender.send_and_wait(move || -> Result<(i64, Arc<SendableTexture<BoxedPixelData>>)> {
                let provider = Arc::new(FrameProvider::new(frame_buffer));
                let texture = Texture::new_with_provider(engine_handle, provider)
                    .map_err(|e| anyhow!("failed to create texture: {e:?}"))?;
                let id = texture.id();
                Ok((id, texture.into_sendable_texture()))
            })?
        };

        let pipeline = build_pipeline(&frame_buffer, &sendable, &emitter)?;

        let player = Self {
            pipeline,
            texture_id,
            frame_buffer,
            emitter,
            rate: Arc::new(Mutex::new(1.0)),
            looping: Arc::new(AtomicBool::new(false)),
            desired_playing: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(true)),
            bus_thread: Mutex::new(None),
            _sendable: sendable,
        };

        player.spawn_bus_thread();
        Ok(player)
    }

    pub fn texture_id(&self) -> i64 {
        self.texture_id
    }

    pub fn set_emitter(&self, emitter: Emitter) {
        *self.emitter.lock() = Some(emitter);
    }

    fn emit(&self, event: PlayerEvent) {
        if let Some(cb) = self.emitter.lock().as_ref() {
            cb(event);
        }
    }

    /// Loads a media URI. Accepts `file://`, `http(s)://`, `rtsp://`, etc.
    pub fn set_uri(&self, uri: &str) -> Result<()> {
        self.pipeline.set_state(gst::State::Ready)?;
        self.pipeline.set_property("uri", uri);
        // Preroll so duration/size become available and the first frame renders.
        self.pipeline.set_state(gst::State::Paused)?;
        Ok(())
    }

    pub fn play(&self) -> Result<()> {
        self.desired_playing.store(true, Ordering::SeqCst);
        self.pipeline.set_state(gst::State::Playing)?;
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.pipeline.set_state(gst::State::Paused)?;
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.desired_playing.store(false, Ordering::SeqCst);
        self.pipeline.set_state(gst::State::Ready)?;
        self.frame_buffer.clear();
        self.emit(PlayerEvent::state(PlayerState::Stopped));
        Ok(())
    }

    pub fn seek(&self, position_ms: i64) -> Result<()> {
        let rate = *self.rate.lock();
        let pos = gst::ClockTime::from_mseconds(position_ms.max(0) as u64);
        self.pipeline.seek(
            rate,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            pos,
            gst::SeekType::None,
            gst::ClockTime::ZERO,
        )?;
        Ok(())
    }

    pub fn set_volume(&self, volume: f64) {
        // playbin exposes a `volume` property in [0.0, 1.0+] and a `mute` flag.
        self.pipeline.set_property("volume", volume.clamp(0.0, 1.0));
    }

    pub fn set_mute(&self, mute: bool) {
        self.pipeline.set_property("mute", mute);
    }

    pub fn set_speed(&self, speed: f64) -> Result<()> {
        let speed = if speed <= 0.0 { 1.0 } else { speed };
        *self.rate.lock() = speed;
        let pos = self
            .pipeline
            .query_position::<gst::ClockTime>()
            .unwrap_or(gst::ClockTime::ZERO);
        self.pipeline.seek(
            speed,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            pos,
            gst::SeekType::None,
            gst::ClockTime::ZERO,
        )?;
        Ok(())
    }

    pub fn set_looping(&self, looping: bool) {
        self.looping.store(looping, Ordering::SeqCst);
    }

    pub fn position_ms(&self) -> i64 {
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map(|p| p.mseconds() as i64)
            .unwrap_or(0)
    }

    pub fn duration_ms(&self) -> i64 {
        self.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|d| d.mseconds() as i64)
            .unwrap_or(0)
    }

    fn spawn_bus_thread(&self) {
        let bus = match self.pipeline.bus() {
            Some(b) => b,
            None => return,
        };
        let pipeline = self.pipeline.clone();
        let emitter = self.emitter.clone();
        let looping = self.looping.clone();
        let desired_playing = self.desired_playing.clone();
        let running = self.running.clone();

        let handle = std::thread::spawn(move || {
            let emit = |event: PlayerEvent| {
                if let Some(cb) = emitter.lock().as_ref() {
                    cb(event);
                }
            };
            let mut last_pos = Instant::now();

            while running.load(Ordering::SeqCst) {
                if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) {
                    use gst::MessageView;
                    match msg.view() {
                        MessageView::Eos(..) => {
                            if looping.load(Ordering::SeqCst) {
                                let _ = pipeline.seek_simple(
                                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                                    gst::ClockTime::ZERO,
                                );
                            } else {
                                emit(PlayerEvent::eos());
                                emit(PlayerEvent::state(PlayerState::Completed));
                            }
                        }
                        MessageView::Error(err) => {
                            emit(PlayerEvent::error(format!(
                                "{} ({:?})",
                                err.error(),
                                err.debug()
                            )));
                            emit(PlayerEvent::state(PlayerState::Error));
                        }
                        MessageView::Buffering(b) => {
                            let percent = b.percent();
                            emit(PlayerEvent::buffering(percent));
                            if desired_playing.load(Ordering::SeqCst) {
                                let target = if percent < 100 {
                                    gst::State::Paused
                                } else {
                                    gst::State::Playing
                                };
                                let _ = pipeline.set_state(target);
                            }
                        }
                        MessageView::DurationChanged(..) => {
                            if let Some(d) =
                                pipeline.query_duration::<gst::ClockTime>()
                            {
                                emit(PlayerEvent::duration(d.mseconds() as i64));
                            }
                        }
                        MessageView::StateChanged(sc) => {
                            if sc
                                .src()
                                .map(|s| s == &pipeline)
                                .unwrap_or(false)
                            {
                                emit(PlayerEvent::state(map_state(sc.current())));
                                if sc.current() == gst::State::Paused
                                    || sc.current() == gst::State::Playing
                                {
                                    if let Some(d) =
                                        pipeline.query_duration::<gst::ClockTime>()
                                    {
                                        emit(PlayerEvent::duration(d.mseconds() as i64));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }

                if last_pos.elapsed() >= Duration::from_millis(200) {
                    last_pos = Instant::now();
                    if let Some(p) = pipeline.query_position::<gst::ClockTime>() {
                        emit(PlayerEvent::position(p.mseconds() as i64));
                    }
                }
            }
        });

        *self.bus_thread.lock() = Some(handle);
    }
}

impl Drop for GstPlayer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.pipeline.set_state(gst::State::Null);
        if let Some(handle) = self.bus_thread.lock().take() {
            let _ = handle.join();
        }
    }
}

fn map_state(state: gst::State) -> PlayerState {
    match state {
        gst::State::Null => PlayerState::Stopped,
        gst::State::Ready => PlayerState::Ready,
        gst::State::Paused => PlayerState::Paused,
        gst::State::Playing => PlayerState::Playing,
        _ => PlayerState::Idle,
    }
}

/// Builds a `playbin3` pipeline whose video output is an RGBA `appsink` wrapped
/// in a `videoconvert` bin. Each decoded frame is copied into `frame_buffer` and
/// the texture is marked dirty.
fn build_pipeline(
    frame_buffer: &Arc<FrameBuffer>,
    sendable: &Arc<SendableTexture<BoxedPixelData>>,
    emitter: &Arc<Mutex<Option<Emitter>>>,
) -> Result<gst::Pipeline> {
    let playbin = gst::ElementFactory::make("playbin3")
        .build()
        .map_err(|_| anyhow!("failed to create playbin3 (is gst-plugins-base installed?)"))?;

    let caps = gst::Caps::builder("video/x-raw").field("format", "RGBA").build();
    let appsink = gst_app::AppSink::builder()
        .caps(&caps)
        .max_buffers(1)
        .drop(true)
        .enable_last_sample(false)
        .build();

    let convert = gst::ElementFactory::make("videoconvert")
        .build()
        .map_err(|_| anyhow!("failed to create videoconvert"))?;

    let sink_bin = gst::Bin::new();
    sink_bin.add_many([&convert, appsink.upcast_ref::<gst::Element>()])?;
    gst::Element::link_many([&convert, appsink.upcast_ref::<gst::Element>()])?;

    let sink_pad = convert
        .static_pad("sink")
        .ok_or_else(|| anyhow!("videoconvert has no sink pad"))?;
    let ghost = gst::GhostPad::with_target(&sink_pad)?;
    ghost.set_active(true)?;
    sink_bin.add_pad(&ghost)?;

    playbin.set_property("video-sink", &sink_bin);

    // Wire the frame callback.
    let fb = frame_buffer.clone();
    let tex = sendable.clone();
    let emitter = emitter.clone();
    let last_size: Arc<Mutex<(i32, i32)>> = Arc::new(Mutex::new((0, 0)));

    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                let caps = sample.caps().ok_or(gst::FlowError::Error)?;
                let info = gst_video::VideoInfo::from_caps(caps)
                    .map_err(|_| gst::FlowError::Error)?;
                let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
                    .map_err(|_| gst::FlowError::Error)?;

                let width = frame.width() as i32;
                let height = frame.height() as i32;
                let src_stride = frame.plane_stride()[0] as usize;
                let plane = frame.plane_data(0).map_err(|_| gst::FlowError::Error)?;
                let row_bytes = width as usize * 4;

                let mut data = vec![0u8; row_bytes * height as usize];
                if src_stride == row_bytes {
                    data.copy_from_slice(&plane[..row_bytes * height as usize]);
                } else {
                    for y in 0..height as usize {
                        let s = y * src_stride;
                        let d = y * row_bytes;
                        data[d..d + row_bytes].copy_from_slice(&plane[s..s + row_bytes]);
                    }
                }

                fb.set(width, height, data);
                tex.mark_frame_available();

                {
                    let mut ls = last_size.lock();
                    if *ls != (width, height) {
                        *ls = (width, height);
                        if let Some(cb) = emitter.lock().as_ref() {
                            cb(PlayerEvent::video_size(width, height));
                        }
                    }
                }

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    let pipeline = playbin
        .dynamic_cast::<gst::Pipeline>()
        .map_err(|_| anyhow!("playbin3 is not a pipeline"))?;
    Ok(pipeline)
}
