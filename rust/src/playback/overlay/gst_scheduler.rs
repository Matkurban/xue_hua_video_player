//! 可注入的 Gst 线程任务调度接缝 / Injectable seam for fire-and-forget Gst-thread work.
//!
//! 将 overlay 绑定、清除等副作用与具体线程派发机制解耦，便于单元测试同步排空任务。
//!
//! Decouples overlay bind/clear side effects from the concrete thread-dispatch mechanism,
//! enabling synchronous task draining in unit tests.

use std::sync::Arc;

use parking_lot::Mutex;

/// 在 Gst 运行时线程上调度任务 / Schedules work on the Gst runtime thread (prod: [`crate::gst::spawn_on_gst_thread`]).
///
/// 实现方必须保证 `spawn` 中的闭包最终在 Gst 专用线程执行。
///
/// Implementors must ensure closures passed to `spawn` ultimately run on the Gst thread.
pub trait GstTaskScheduler: Send + Sync {
    /// 异步派发一次性任务 / Dispatches a one-shot task asynchronously.
    ///
    /// # 参数 / Parameters
    /// - `task` — 在 Gst 线程上执行的 `FnOnce` 闭包 / `FnOnce` closure to run on the Gst thread
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>);
}

/// 生产环境调度器 — 委托给 `spawn_on_gst_thread` / Production scheduler — delegates to `spawn_on_gst_thread`.
pub struct SpawnOnGstThreadScheduler;

impl GstTaskScheduler for SpawnOnGstThreadScheduler {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        crate::gst::spawn_on_gst_thread(task);
    }
}

/// 测试用调度器 — 将任务入队以便同步排空 / Test scheduler — queues tasks for synchronous draining.
#[cfg(test)]
pub struct SyncGstTaskScheduler {
    tasks: Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>,
}

#[cfg(test)]
impl SyncGstTaskScheduler {
    /// 创建空的同步调度器 / Creates an empty synchronous scheduler.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 返回尚未执行的任务数量 / Returns the number of tasks not yet executed.
    ///
    /// # 返回值 / Returns
    /// - 队列中待排空任务数 / Count of pending queued tasks
    pub fn pending_count(&self) -> usize {
        self.tasks.lock().len()
    }

    /// 同步执行队列中全部任务（LIFO 顺序弹出）/ Synchronously runs all queued tasks (popped LIFO).
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
