//! Platform overlay adapters (Android, iOS, desktop/macOS window).

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(any(
    target_os = "macos",
    all(
        not(target_os = "android"),
        not(target_os = "macos"),
        not(target_os = "ios")
    )
))]
pub mod window;
