package xue_hua.video_player;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

import android.content.Context;
import android.content.res.Resources;
import android.util.DisplayMetrics;
import android.view.Surface;

import io.flutter.view.TextureRegistry;

import org.junit.After;
import org.junit.Before;
import org.junit.Test;

import java.util.ArrayList;
import java.util.List;

/** Regression: SurfaceProducer bind / resize ordering for GStreamer VideoOverlay. */
public class XueHuaVideoTextureTest {
    /** Unified timeline: bridge + producer callbacks in call order. */
    private final List<String> events = new ArrayList<>();

    @Before
    public void installBridgeListener() {
        events.clear();
        XueHuaVideoTexture.rebindScheduler = Runnable::run;
        AndroidSurfaceBridge.listener =
            new AndroidSurfaceBridge.Listener() {
                @Override
                public void onSurfaceChanged(
                    long playerId, Surface surface, int width, int height) {
                    events.add("changed");
                }

                @Override
                public void onSurfaceDestroyed(long playerId) {
                    events.add("destroyed");
                }
            };
    }

    @After
    public void clearBridgeListener() {
        AndroidSurfaceBridge.listener = null;
        XueHuaVideoTexture.rebindScheduler = null;
    }

    @Test
    public void constructor_bindsSurfaceImmediatelyAfterSetCallback() {
        FakeRegistry registry = new FakeRegistry();
        registry.producer.surface = validSurface();
        new XueHuaVideoTexture(1L, registry);

        assertNotNull(registry.producer.callback);
        assertTrue(
            "getSurface() must run in constructor so GStreamer has a window before PLAYING",
            registry.producer.getSurfaceCallCount >= 1);
        assertTrue(events.contains("changed"));
    }

    @Test
    public void constructor_withContext_eagerSetSizeFromDisplayMetrics() {
        FakeRegistry registry = new FakeRegistry();
        registry.producer.surface = validSurface();
        Context context = mockDisplayContext(1080, 1920);

        new XueHuaVideoTexture(1L, registry, context);

        // Portrait 1080×1920 contain-fit 16:9 → 1080×608.
        assertEquals(1080, registry.producer.width);
        assertEquals(608, registry.producer.height);
        assertEquals(1, registry.producer.setSizeCallCount);
        assertTrue(events.contains("changed"));
    }

    @Test
    public void containFit16x9_landscape() {
        int[] size = XueHuaVideoTexture.containFit16x9(1920, 1080);
        assertNotNull(size);
        assertEquals(1920, size[0]);
        assertEquals(1080, size[1]);
    }

    @Test
    public void containFit16x9_rejectsTiny() {
        assertNull(XueHuaVideoTexture.containFit16x9(1, 1));
        assertNull(XueHuaVideoTexture.containFit16x9(0, 100));
    }

    @Test
    public void syncSize_unbindsBeforeSetSizeThenRebinds() {
        FakeRegistry registry = new FakeRegistry();
        registry.producer.surface = validSurface();
        XueHuaVideoTexture texture = new XueHuaVideoTexture(1L, registry);
        events.clear();
        registry.producer.destroyOnSetSize = true;

        texture.syncSize(1080, 608);

        assertEquals(1080, registry.producer.width);
        assertEquals(608, registry.producer.height);
        assertEquals(1, registry.producer.setSizeCallCount);
        assertTrue(events.contains("setSize"));
        assertTrue(events.contains("destroyed"));
        assertTrue(events.contains("changed"));
        assertTrue(
            "must unbind VideoOverlay before setSize destroys the old Surface",
            indexOfEvent(events, "destroyed") < indexOfEvent(events, "setSize"));
        assertTrue(
            "rebind must happen after setSize",
            indexOfEvent(events, "setSize") < lastIndexOfEvent(events, "changed"));
    }

    @Test
    public void syncSize_sameDimensionsStillRebindsWhenUnbound() {
        FakeRegistry registry = new FakeRegistry();
        registry.producer.surface = validSurface();
        XueHuaVideoTexture texture = new XueHuaVideoTexture(1L, registry);
        texture.syncSize(800, 450);
        // Real cleanup: producer no longer has a valid surface.
        registry.producer.surface = null;
        registry.producer.callback.onSurfaceCleanup();
        registry.producer.surface = validSurface();
        events.clear();
        int afterFirst = registry.producer.setSizeCallCount;
        int surfacesAfterFirst = registry.producer.getSurfaceCallCount;

        texture.syncSize(800, 450);

        assertEquals(afterFirst, registry.producer.setSizeCallCount);
        assertTrue(
            "duplicate syncSize must still re-bind in case native window was cleared",
            registry.producer.getSurfaceCallCount > surfacesAfterFirst);
        assertTrue(events.contains("changed"));
    }

    @Test
    public void onSurfaceCleanup_clearsThenRebinds() {
        FakeRegistry registry = new FakeRegistry();
        registry.producer.surface = validSurface();
        XueHuaVideoTexture texture = new XueHuaVideoTexture(1L, registry);
        texture.syncSize(1080, 608);
        events.clear();

        registry.producer.callback.onSurfaceCleanup();

        assertTrue(
            "cleanup must always clear the bound window",
            events.contains("destroyed"));
        assertTrue(
            "posted rebind must restore the window via getSurface",
            events.contains("changed"));
        assertTrue(
            indexOfEvent(events, "destroyed") < indexOfEvent(events, "changed"));
    }

    @Test
    public void syncSize_sameSurfaceSkipsRedundantRebind() {
        FakeRegistry registry = new FakeRegistry();
        registry.producer.surface = validSurface();
        XueHuaVideoTexture texture = new XueHuaVideoTexture(1L, registry);
        texture.syncSize(800, 450);
        events.clear();
        int surfacesAfterFirst = registry.producer.getSurfaceCallCount;

        texture.syncSize(800, 450);

        assertTrue(registry.producer.getSurfaceCallCount > surfacesAfterFirst);
        assertFalse(
            "same Surface+size must not call onSurfaceChanged again",
            events.contains("changed"));
    }

    @Test
    public void syncSize_duringResize_cleanupDoesNotDestroyAfterRebind() {
        FakeRegistry registry = new FakeRegistry();
        registry.producer.surface = validSurface();
        XueHuaVideoTexture texture = new XueHuaVideoTexture(1L, registry);
        registry.producer.destroyOnSetSize = true;
        events.clear();

        texture.syncSize(1280, 720);

        int destroyCount = 0;
        for (String step : events) {
            if ("destroyed".equals(step)) {
                destroyCount++;
            }
        }
        assertEquals(
            "cleanup during setSize must not destroy after the new surface is bound",
            1,
            destroyCount);
        assertTrue(events.contains("changed"));
        assertFalse(
            "last event must not be a stale destroy after rebind",
            "destroyed".equals(events.get(events.size() - 1)));
        assertEquals(1280, registry.producer.width);
    }

    private static Context mockDisplayContext(int widthPx, int heightPx) {
        Context context = mock(Context.class);
        Resources resources = mock(Resources.class);
        DisplayMetrics metrics = new DisplayMetrics();
        metrics.widthPixels = widthPx;
        metrics.heightPixels = heightPx;
        when(context.getResources()).thenReturn(resources);
        when(resources.getDisplayMetrics()).thenReturn(metrics);
        return context;
    }

    private static Surface validSurface() {
        Surface surface = mock(Surface.class);
        when(surface.isValid()).thenReturn(true);
        return surface;
    }

    private static int indexOfEvent(List<String> timeline, String name) {
        int idx = timeline.indexOf(name);
        assertTrue(name + " missing in " + timeline, idx >= 0);
        return idx;
    }

    private static int lastIndexOfEvent(List<String> timeline, String name) {
        int idx = timeline.lastIndexOf(name);
        assertTrue(name + " missing in " + timeline, idx >= 0);
        return idx;
    }

    private final class FakeRegistry implements TextureRegistry {
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

    private final class FakeProducer implements TextureRegistry.SurfaceProducer {
        int getSurfaceCallCount = 0;
        int setSizeCallCount = 0;
        int width = 1;
        int height = 1;
        boolean destroyOnSetSize = false;
        Surface surface;
        TextureRegistry.SurfaceProducer.Callback callback;

        @Override
        public long id() {
            return 42L;
        }

        @Override
        public void release() {}

        @Override
        public void setSize(int width, int height) {
            events.add("setSize");
            setSizeCallCount++;
            this.width = width;
            this.height = height;
            if (destroyOnSetSize && callback != null) {
                callback.onSurfaceCleanup();
            }
        }

        @Override
        public int getWidth() {
            return width;
        }

        @Override
        public int getHeight() {
            return height;
        }

        @Override
        public Surface getSurface() {
            getSurfaceCallCount++;
            return surface;
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
            return surface;
        }
    }
}
