use std::sync::Arc;

use irondash_texture::{BoxedPixelData, PayloadProvider, SimplePixelData};
use parking_lot::Mutex;

/// A single decoded RGBA8888 frame, tightly packed (no row padding).
struct Frame {
    width: i32,
    height: i32,
    data: Vec<u8>,
}

/// Shared, lock-guarded slot holding the most recent decoded frame.
///
/// The GStreamer streaming thread writes into it via [`FrameBuffer::set`], while
/// Flutter's raster thread reads from it through [`FrameProvider::get_payload`].
/// Only the latest frame is retained; older frames are dropped.
#[derive(Default)]
pub struct FrameBuffer {
    inner: Mutex<Option<Frame>>,
}

impl FrameBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(None),
        })
    }

    /// Stores `data` (expected to be tightly packed RGBA8888) as the latest frame.
    pub fn set(&self, width: i32, height: i32, data: Vec<u8>) {
        *self.inner.lock() = Some(Frame {
            width,
            height,
            data,
        });
    }

    /// Drops the current frame so the texture renders transparent again.
    pub fn clear(&self) {
        *self.inner.lock() = None;
    }
}

/// irondash payload provider that hands Flutter the latest frame on demand.
pub struct FrameProvider {
    buffer: Arc<FrameBuffer>,
}

impl FrameProvider {
    pub fn new(buffer: Arc<FrameBuffer>) -> Self {
        Self { buffer }
    }
}

impl PayloadProvider<BoxedPixelData> for FrameProvider {
    fn get_payload(&self) -> BoxedPixelData {
        let guard = self.buffer.inner.lock();
        match guard.as_ref() {
            Some(frame) => {
                SimplePixelData::new_boxed(frame.width, frame.height, frame.data.clone())
            }
            // No frame yet: render a 1x1 transparent pixel.
            None => SimplePixelData::new_boxed(1, 1, vec![0, 0, 0, 0]),
        }
    }
}
