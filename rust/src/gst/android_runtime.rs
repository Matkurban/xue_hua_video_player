//! Android `xhvp-gst` runtime without GLib `MainLoop` / `MainContext`.
//!
//! Uses an `mpsc` command queue plus `gst_bus_timed_pop` polling so we never call
//! `g_main_loop_run` or allocate GLib `GPrivate` TLS keys on SDK-heavy apps.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Once};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::Mutex as ParkingMutex;

use crate::playback::bus::{dispatch_gst_bus_message, BusHandlerState, Emitter};

type GstCommand = Box<dyn FnOnce() + Send>;

static RUNTIME_STARTED: Once = Once::new();
static CMD_TX: OnceCell<mpsc::Sender<GstCommand>> = OnceCell::new();
static GST_THREAD_ID: OnceCell<ThreadId> = OnceCell::new();
static BUS_ENTRIES: ParkingMutex<Vec<BusPollEntry>> = ParkingMutex::new(Vec::new());
static POSITION_ENTRIES: ParkingMutex<Vec<PositionPollEntry>> = ParkingMutex::new(Vec::new());
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

const POLL_BLOCK_MS: u64 = 10;
const POSITION_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Opaque token for an Android bus poll registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusPollToken(u64);

/// Opaque token for an Android position poll registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionPollToken(u64);

struct BusPollEntry {
    token: u64,
    bus: gst::Bus,
    state: BusHandlerState,
}

struct PositionPollEntry {
    token: u64,
    pipeline: gst::Pipeline,
    emitter: std::sync::Arc<ParkingMutex<Option<Emitter>>>,
    running: std::sync::Arc<AtomicBool>,
    last_poll: Instant,
}

impl Drop for BusPollToken {
    fn drop(&mut self) {
        unregister_bus_poll(self.clone());
    }
}

impl Drop for PositionPollToken {
    fn drop(&mut self) {
        unregister_position_poll(self.clone());
    }
}

fn next_token() -> u64 {
    NEXT_TOKEN.fetch_add(1, Ordering::Relaxed)
}

fn on_gst_thread() -> bool {
    GST_THREAD_ID
        .get()
        .is_some_and(|id| *id == std::thread::current().id())
}

/// Starts the process-wide `xhvp-gst` poll-loop thread (idempotent).
pub fn ensure_gst_runtime() {
    RUNTIME_STARTED.call_once(|| {
        let (ready_tx, ready_rx) = mpsc::sync_channel::<()>(1);
        std::thread::Builder::new()
            .name("xhvp-gst".into())
            .spawn(move || gst_runtime_thread_main(ready_tx))
            .expect("xhvp-gst thread spawn");

        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(()) => {
                crate::diag::logcat_info("gst: Gst runtime thread started (Android poll loop)")
            }
            Err(e) => {
                crate::diag::logcat_error(&format!("gst: Gst runtime thread failed to start: {e}"))
            }
        }
    });
}

fn gst_runtime_thread_main(ready_tx: mpsc::SyncSender<()>) {
    let _ = GST_THREAD_ID.set(std::thread::current().id());

    if let Err(e) = crate::platform::android::attach_java_vm() {
        crate::diag::logcat_error(&format!("xhvp-gst: JavaVM attach failed: {e:#}"));
    }

    let already_initialized =
        unsafe { gstreamer::ffi::gst_is_initialized() != gstreamer::glib::ffi::GFALSE };
    if !already_initialized {
        if let Err(e) = gstreamer::init() {
            crate::diag::logcat_error(&format!(
                "gst: gst::init() on gst thread failed: {e} — continuing anyway"
            ));
        }
    }

    let (cmd_tx, cmd_rx) = mpsc::channel::<GstCommand>();
    let _ = CMD_TX.set(cmd_tx);
    let _ = ready_tx.send(());

    let block_timeout = gst::ClockTime::from_mseconds(POLL_BLOCK_MS);

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            cmd();
        }

        poll_position_entries();

        let mut had_bus_work = false;
        {
            let entries = BUS_ENTRIES.lock();
            for entry in entries.iter() {
                while let Some(msg) = entry.bus.pop() {
                    had_bus_work = true;
                    dispatch_gst_bus_message(&entry.state, &msg);
                }
            }
        }

        if !had_bus_work {
            let entries = BUS_ENTRIES.lock();
            if let Some(entry) = entries.first() {
                if let Some(msg) = entry.bus.timed_pop(block_timeout) {
                    dispatch_gst_bus_message(&entry.state, &msg);
                    continue;
                }
            } else {
                std::thread::sleep(Duration::from_millis(POLL_BLOCK_MS));
            }
        }
    }
}

fn poll_position_entries() {
    let now = Instant::now();
    let mut entries = POSITION_ENTRIES.lock();
    for entry in entries.iter_mut() {
        if !entry.running.load(Ordering::SeqCst) {
            continue;
        }
        if now.duration_since(entry.last_poll) < POSITION_POLL_INTERVAL {
            continue;
        }
        entry.last_poll = now;
        let (_, current, _) = entry.pipeline.state(gst::ClockTime::ZERO);
        if current != gst::State::Playing && current != gst::State::Paused {
            continue;
        }
        if let Some(cb) = entry.emitter.lock().as_ref() {
            if let Some(p) = entry.pipeline.query_position::<gst::ClockTime>() {
                cb(crate::player_events::PlayerEvent::position(p.mseconds() as i64));
            }
        }
    }
}

pub fn spawn_on_gst_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    if on_gst_thread() {
        f();
        return;
    }
    ensure_gst_runtime();
    let Some(tx) = CMD_TX.get() else {
        crate::diag::logcat_error("spawn_on_gst_thread: Gst runtime not ready");
        return;
    };
    if tx.send(Box::new(f)).is_err() {
        crate::diag::logcat_error("spawn_on_gst_thread: command channel closed");
    }
}

pub fn spawn_on_gst_thread_and_wait<F, R>(f: F) -> Result<R>
where
    F: FnOnce() -> Result<R> + Send + 'static,
    R: Send + 'static,
{
    if on_gst_thread() {
        return f();
    }
    ensure_gst_runtime();
    let tx = CMD_TX
        .get()
        .ok_or_else(|| anyhow!("Gst runtime command channel not ready"))?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    tx.send(Box::new(move || {
        let _ = reply_tx.send(f());
    }))
    .map_err(|e| anyhow!("Gst thread command send failed: {e}"))?;
    reply_rx
        .recv()
        .map_err(|e| anyhow!("Gst thread reply dropped: {e}"))?
}

pub fn run_on_gst_thread<F, R>(f: F) -> Result<R>
where
    F: FnOnce() -> Result<R> + Send + 'static,
    R: Send + 'static,
{
    spawn_on_gst_thread_and_wait(f)
}

pub(crate) fn register_bus_handlers(
    bus: gst::Bus,
    state: BusHandlerState,
    pipeline: gst::Pipeline,
    emitter: std::sync::Arc<ParkingMutex<Option<Emitter>>>,
    running: std::sync::Arc<AtomicBool>,
) -> (BusPollToken, PositionPollToken) {
    let bus_token = next_token();
    BUS_ENTRIES.lock().push(BusPollEntry {
        token: bus_token,
        bus,
        state,
    });

    let pos_token = next_token();
    POSITION_ENTRIES.lock().push(PositionPollEntry {
        token: pos_token,
        pipeline,
        emitter,
        running,
        last_poll: Instant::now(),
    });

    (BusPollToken(bus_token), PositionPollToken(pos_token))
}

pub fn unregister_bus_poll(token: BusPollToken) {
    BUS_ENTRIES.lock().retain(|e| e.token != token.0);
}

pub fn unregister_position_poll(token: PositionPollToken) {
    POSITION_ENTRIES.lock().retain(|e| e.token != token.0);
}
