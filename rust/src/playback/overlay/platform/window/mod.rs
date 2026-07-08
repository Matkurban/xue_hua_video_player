mod backend;
mod session;

#[cfg(target_os = "macos")]
pub use backend::MacosOverlayBackend;
#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
pub use backend::{apply_overlay_handle, DesktopOverlayBackend};
#[cfg(all(
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
pub use session::DesktopOverlaySession;
#[cfg(target_os = "macos")]
pub use session::MacosOverlaySession;
