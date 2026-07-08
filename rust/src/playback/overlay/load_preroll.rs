//! Load/switch preroll policy — single `decide_preroll_action` with `want_play=false`.

use anyhow::Result;
use gstreamer as gst;

use crate::playback::overlay::preroll_gate::{decide_preroll_action, PipelineSnapshot, PrerollAction};
use crate::playback::shell::PipelineShell;
use crate::playback::state::set_state_sync;
use crate::playback::surface::VideoSurface;

#[cfg(target_os = "android")]
use crate::playback::overlay::refresh_mobile_overlay_on_gst;

/// Platform policy for URI/asset **load** preroll (distinct from bind-path [`super::preroll_executor`]).
pub trait LoadPrerollPolicy {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool;
    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface_overlay_ready: bool,
        surface: &VideoSurface,
        defer_log: &str,
    ) -> Result<()>;
}

/// Android: pass-through overlay readiness; may pause preroll + refresh overlay.
#[cfg(target_os = "android")]
pub struct AndroidLoadPrerollPolicy;

#[cfg(target_os = "android")]
impl LoadPrerollPolicy for AndroidLoadPrerollPolicy {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
        surface_overlay_ready
    }

    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface_overlay_ready: bool,
        surface: &VideoSurface,
        defer_log: &str,
    ) -> Result<()> {
        let gate_ready = self.gate_ready_for_load(surface_overlay_ready);
        let snapshot = PipelineSnapshot::from_shell(shell);
        match decide_preroll_action(snapshot, false, gate_ready) {
            PrerollAction::PausePreroll => {
                set_state_sync(&shell.pipeline, gst::State::Paused)?;
                if let Some(handle) = *surface.stored_handle().lock() {
                    let (width, height) = surface.cached_dimensions();
                    refresh_mobile_overlay_on_gst(
                        shell,
                        handle,
                        width,
                        height,
                        "after Paused preroll",
                    )?;
                }
            }
            PrerollAction::Defer => {
                crate::diag::logcat_info(defer_log);
            }
            PrerollAction::Noop | PrerollAction::ResumePlaying => {}
        }
        Ok(())
    }
}

/// iOS: load never prerolls on switch — attach deferred to [`super::ios_session::IosOverlaySession`].
#[cfg(target_os = "ios")]
pub struct IosLoadPrerollPolicy;

#[cfg(target_os = "ios")]
impl LoadPrerollPolicy for IosLoadPrerollPolicy {
    fn gate_ready_for_load(&self, _surface_overlay_ready: bool) -> bool {
        false
    }

    fn apply_load_preroll(
        &self,
        _shell: &PipelineShell,
        surface_overlay_ready: bool,
        _surface: &VideoSurface,
        defer_log: &str,
    ) -> Result<()> {
        if surface_overlay_ready {
            log::debug!("gst: ios layer attach deferred to IosOverlaySession after load");
        } else {
            log::info!("{defer_log}");
        }
        Ok(())
    }
}

/// Desktop/macOS: preroll when handle is not cached yet.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub struct DesktopLoadPrerollPolicy;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl LoadPrerollPolicy for DesktopLoadPrerollPolicy {
    fn gate_ready_for_load(&self, surface_overlay_ready: bool) -> bool {
        !surface_overlay_ready
    }

    fn apply_load_preroll(
        &self,
        shell: &PipelineShell,
        surface_overlay_ready: bool,
        _surface: &VideoSurface,
        _defer_log: &str,
    ) -> Result<()> {
        let gate_ready = self.gate_ready_for_load(surface_overlay_ready);
        let snapshot = PipelineSnapshot::from_shell(shell);
        if decide_preroll_action(snapshot, false, gate_ready) == PrerollAction::PausePreroll {
            set_state_sync(&shell.pipeline, gst::State::Paused)?;
        }
        Ok(())
    }
}

/// Resolves the platform load preroll policy.
pub fn platform_load_preroll_policy() -> impl LoadPrerollPolicy {
    #[cfg(target_os = "android")]
    {
        return AndroidLoadPrerollPolicy;
    }
    #[cfg(target_os = "ios")]
    {
        return IosLoadPrerollPolicy;
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        DesktopLoadPrerollPolicy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "android")]
    #[test]
    fn android_gate_passes_surface_ready() {
        let policy = AndroidLoadPrerollPolicy;
        assert!(policy.gate_ready_for_load(true));
        assert!(!policy.gate_ready_for_load(false));
    }

    #[cfg(target_os = "ios")]
    #[test]
    fn ios_gate_always_false() {
        let policy = IosLoadPrerollPolicy;
        assert!(!policy.gate_ready_for_load(true));
        assert!(!policy.gate_ready_for_load(false));
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn desktop_gate_inverts_ready() {
        let policy = DesktopLoadPrerollPolicy;
        assert!(policy.gate_ready_for_load(false));
        assert!(!policy.gate_ready_for_load(true));
    }
}
