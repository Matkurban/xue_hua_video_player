#[cfg(target_os = "android")]
use anyhow::Result;

#[cfg(target_os = "android")]
use super::resolver::AssetByteSource;

#[cfg(target_os = "android")]
pub fn can_open_asset_fd(asset_key: &str) -> bool {
    open_asset_fd(asset_key).is_ok()
}

/// Opens a Flutter asset via `AssetManager.openFd` and returns a byte source.
#[cfg(target_os = "android")]
pub fn open_asset_fd(asset_key: &str) -> Result<AssetByteSource> {
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    let (fd, start, length) = crate::platform_view_android::with_jni_env(|env| {
        crate::platform_view_android::call_open_asset_fd(env, asset_key)
    })
    .map_err(|e| {
        crate::diag::logcat_error(&format!("open_asset_fd JNI failed for {asset_key}: {e:#}"));
        e
    })?;

    // SAFETY: fd detached from AssetFileDescriptor in Java helper.
    let file = unsafe { File::from_raw_fd(fd) };
    Ok(AssetByteSource::AndroidFd {
        file,
        start,
        length,
        position: 0,
    })
}
