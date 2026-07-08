//! 共享 overlay 后端契约（供 [`super::super::surface::VideoSurface`] 与测试使用）。
//!
//! 定义跨平台共用的原生窗口句柄缓存与预卷就绪判定最小接口。
//!
//! Shared overlay backend contract (used by [`super::super::surface::VideoSurface`] and tests).
//!
//! Defines the minimal cross-platform interface for native window handle caching
//! and preroll-readiness checks.

use parking_lot::Mutex;

/// 跨平台共用的最小 overlay 状态面 / Minimal overlay state surface shared across platforms.
///
/// 实现方提供已缓存原生句柄的互斥槽；默认方法处理句柄读写与预卷门控。
///
/// Implementors supply a mutex slot for the cached native handle; default methods
/// handle handle read/write and preroll gating.
pub trait VideoOverlayBackend {
    /// 返回已缓存原生句柄的互斥引用 / Returns the mutex reference for the cached native handle.
    ///
    /// # 返回值 / Returns
    /// - 指向 `Option<usize>` 的互斥锁；`Some` 表示句柄已缓存 / Mutex over `Option<usize>`; `Some` means cached
    fn stored_handle(&self) -> &Mutex<Option<usize>>;

    /// 缓存或清除原生 overlay 句柄 / Caches or clears the native overlay handle.
    ///
    /// # 参数 / Parameters
    /// - `handle` — 原生句柄；`0` 表示销毁/解绑并清除缓存 / native handle; `0` clears the cache
    fn cache_handle(&self, handle: usize) {
        if handle == 0 {
            self.stored_handle().lock().take();
        } else {
            *self.stored_handle().lock() = Some(handle);
        }
    }

    /// 是否已缓存非零原生句柄 / Whether a non-zero native handle is cached.
    ///
    /// # 返回值 / Returns
    /// - `true` 当互斥槽为 `Some` / `true` when the mutex slot is `Some`
    fn has_cached_handle(&self) -> bool {
        self.stored_handle().lock().is_some()
    }

    /// 预卷是否可继续（平台特定的绑定规则）/ True when preroll may proceed (platform-specific bind rules).
    ///
    /// 默认实现仅要求已缓存句柄；平台实现可叠加 GStreamer 绑定状态。
    ///
    /// Default requires only a cached handle; platform impls may also require GStreamer bind.
    ///
    /// # 返回值 / Returns
    /// - `true` 当满足该平台预卷门控 / `true` when this platform's preroll gate passes
    fn overlay_ready_for_preroll(&self) -> bool {
        self.has_cached_handle()
    }
}
