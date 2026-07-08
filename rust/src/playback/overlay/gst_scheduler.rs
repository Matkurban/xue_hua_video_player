//! Injectable seam for fire-and-forget Gst-thread work.

use std::sync::Arc;

use parking_lot::Mutex;

/// Schedules work on the Gst runtime thread (prod: [`crate::gst_runtime::spawn_on_gst_thread`]).
pub trait GstTaskScheduler: Send + Sync {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>);
}

/// Production scheduler — delegates to `spawn_on_gst_thread`.
pub struct SpawnOnGstThreadScheduler;

impl GstTaskScheduler for SpawnOnGstThreadScheduler {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        crate::gst_runtime::spawn_on_gst_thread(task);
    }
}

/// Test scheduler — queues tasks for synchronous draining.
#[cfg(test)]
pub struct SyncGstTaskScheduler {
    tasks: Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>,
}

#[cfg(test)]
impl SyncGstTaskScheduler {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn pending_count(&self) -> usize {
        self.tasks.lock().len()
    }

    pub fn drain(&self) {
        let mut pending = self.tasks.lock();
        while let Some(task) = pending.pop() {
            task();
        }
    }
}

#[cfg(test)]
impl GstTaskScheduler for SyncGstTaskScheduler {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        self.tasks.lock().push(task);
    }
}
