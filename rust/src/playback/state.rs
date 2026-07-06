use anyhow::{anyhow, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer::StateChangeSuccess;

const DEFAULT_STATE_TIMEOUT: gst::ClockTime = gst::ClockTime::from_seconds(10);

/// Sets pipeline/element state and waits until the transition completes.
pub fn set_state_sync(
    element: &impl IsA<gst::Element>,
    target: gst::State,
) -> Result<()> {
    set_state_sync_timeout(element, target, DEFAULT_STATE_TIMEOUT)
}

pub fn set_state_sync_timeout(
    element: &impl IsA<gst::Element>,
    target: gst::State,
    timeout: gst::ClockTime,
) -> Result<()> {
    let element = element.upcast_ref::<gst::Element>();
    let change = element
        .set_state(target)
        .map_err(|e| anyhow!("set_state({target:?}) failed: {e}"))?;
    if matches!(change, StateChangeSuccess::Success) {
        return Ok(());
    }
    let (ret, current, _pending) = element.state(Some(timeout));
    ret.map_err(|e| anyhow!("get_state after set_state({target:?}) failed: {e}"))?;
    if current != target {
        log::error!(
            "pipeline failed to reach {target:?} within {:?}, stuck in {current:?}",
            timeout
        );
        return Err(anyhow!(
            "element failed to change state to {target:?} (current {current:?})"
        ));
    }
    Ok(())
}
