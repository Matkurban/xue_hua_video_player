//! Android 上通过 JNI `AssetManager.openFd` 打开 Flutter 资源。
//!
//! Opens Flutter assets via JNI `AssetManager.openFd` on Android.
//!
//! 返回的文件描述符区间供 [`super::resolver::AssetByteSource::AndroidFd`] 在
//! AppSrc `need-data` 回调中按偏移读取。

#[cfg(target_os = "android")]
use anyhow::Result;

#[cfg(target_os = "android")]
use super::resolver::AssetByteSource;

/// 探测能否通过 `AssetManager.openFd` 打开指定 Flutter 资源。
/// Probes whether the given Flutter asset can be opened via `AssetManager.openFd`.
///
/// # 参数 / Parameters
/// - `asset_key` — Flutter 资源键 / Flutter asset key.
///
/// # 返回值 / Returns
/// - 可打开时返回 `true` / `true` when openable.
///
/// # 平台 / Platform
/// - 仅 **Android** / **Android** only.
#[cfg(target_os = "android")]
pub fn can_open_asset_fd(asset_key: &str) -> bool {
    open_asset_fd(asset_key).is_ok()
}

/// 通过 `AssetManager.openFd` 打开 Flutter 资源并返回字节源。
/// Opens a Flutter asset via `AssetManager.openFd` and returns a byte source.
///
/// # 参数 / Parameters
/// - `asset_key` — Flutter 资源键 / Flutter asset key.
///
/// # 返回值 / Returns
/// - [`AssetByteSource::AndroidFd`]，携带 fd、起始偏移与长度 / Variant with fd, start
///   offset, and length.
///
/// # 错误 / Errors
/// - JNI 调用失败或 Java 辅助类无法打开资源时失败 / Fails on JNI errors or when the
///   Java helper cannot open the asset.
///
/// # 平台 / Platform
/// - 仅 **Android** / **Android** only.
#[cfg(target_os = "android")]
pub fn open_asset_fd(asset_key: &str) -> Result<AssetByteSource> {
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    let (fd, start, length) = crate::platform::android::with_jni_env(|env| {
        crate::platform::android::call_open_asset_fd(env, asset_key)
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
