package com.flutter_rust_bridge.xue_hua_video_player;

import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

import android.view.Surface;

import io.flutter.view.TextureRegistry;

import org.junit.Test;

/** Regression: initial bind must follow setCallback (TextureVideoPlayer pattern). */
public class XueHuaVideoTextureTest {
    @Test
    public void constructor_getsSurfaceImmediatelyAfterSetCallback() {
        FakeRegistry registry = new FakeRegistry();
        new XueHuaVideoTexture(1L, registry);

        assertNotNull(registry.producer.callback);
        assertTrue(
            "getSurface() must run in constructor so GStreamer binds before onSurfaceAvailable",
            registry.producer.getSurfaceCallCount >= 1);
    }

    private static final class FakeRegistry implements TextureRegistry {
        final FakeProducer producer = new FakeProducer();

        @Override
        public SurfaceProducer createSurfaceProducer() {
            return producer;
        }

        @Override
        public SurfaceProducer createSurfaceProducer(SurfaceLifecycle lifecycle) {
            return createSurfaceProducer();
        }

        @Override
        public SurfaceTextureEntry createSurfaceTexture() {
            throw new UnsupportedOperationException();
        }

        @Override
        public SurfaceTextureEntry registerSurfaceTexture(
            android.graphics.SurfaceTexture surfaceTexture) {
            throw new UnsupportedOperationException();
        }

        @Override
        public ImageTextureEntry createImageTexture() {
            throw new UnsupportedOperationException();
        }
    }

    private static final class FakeProducer implements TextureRegistry.SurfaceProducer {
        int getSurfaceCallCount = 0;
        TextureRegistry.SurfaceProducer.Callback callback;

        @Override
        public long id() {
            return 42L;
        }

        @Override
        public void release() {}

        @Override
        public void setSize(int width, int height) {}

        @Override
        public int getWidth() {
            return 1;
        }

        @Override
        public int getHeight() {
            return 1;
        }

        @Override
        public Surface getSurface() {
            getSurfaceCallCount++;
            return null;
        }

        @Override
        public void setCallback(Callback callback) {
            this.callback = callback;
        }

        @Override
        public void scheduleFrame() {}

        @Override
        public boolean handlesCropAndRotation() {
            return false;
        }

        @Override
        public Surface getForcedNewSurface() {
            getSurfaceCallCount++;
            return null;
        }
    }
}
