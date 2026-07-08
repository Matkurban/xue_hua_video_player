//! 媒体源解析：将 Dart 传入的 URI 或 Flutter 资源键转换为 GStreamer 可消费的输入。
//!
//! Media source resolution: converts Dart-supplied URIs or Flutter asset keys into
//! GStreamer-consumable inputs.
//!
//! [`MediaSource`] 描述高层输入；[`resolve`] 产出 [`ResolvedSource`]（直接 URI 或
//! AppSrc 推送），供 [`crate::playback::PlaybackEngine::load`] 接线管线。

#[cfg(target_os = "android")]
mod asset_android;
mod resolver;

pub use resolver::{resolve_flutter_asset_path, set_flutter_assets_dir, AppSrcFeedState};

use anyhow::{anyhow, Result};

/// 播放引擎 [`crate::playback::PlaybackEngine::load`] 接受的媒体输入。
/// Media input accepted by [`crate::playback::PlaybackEngine::load`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaSource {
    /// 网络或本地 URI：`http(s)://`、`rtsp://`、`file://`、HLS 等。
    /// Network or local URI: `http(s)://`, `rtsp://`, `file://`, HLS, etc.
    Uri(String),
    /// Flutter 资源键，例如 `assets/sample.mp4`。
    /// Flutter asset key, e.g. `assets/sample.mp4`.
    FlutterAsset(String),
}

/// [`MediaSource`] 解析后接入 GStreamer 的方式。
/// How a [`MediaSource`] is wired into GStreamer after resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSource {
    /// 可直接传给 `playbin` 等元素的 URI 字符串。
    /// URI string passable directly to `playbin` and similar elements.
    Uri(String),
    /// 通过 AppSrc `need-data` 回调按字节推送的资源键。
    /// Asset key fed byte-by-byte via AppSrc `need-data` callbacks.
    AppSrc(String),
}

impl MediaSource {
    /// 规范化网络/文件 URI，或将 Flutter 资源解析为最佳播放路径。
    /// Normalizes network/file URIs and resolves Flutter assets to the best playback path.
    ///
    /// # 参数 / Parameters
    /// 无（消费 `self`）/ None (consumes `self`).
    ///
    /// # 返回值 / Returns
    /// - [`ResolvedSource::Uri`] 或 [`ResolvedSource::AppSrc`] / Either variant.
    ///
    /// # 错误 / Errors
    /// - 空 URI/资源键、路径无效或 Android 资源不可用时失败。
    ///   Fails on empty URI/asset key, invalid path, or unavailable Android asset.
    ///
    /// # 平台 / Platform
    /// - **Android**：Flutter 资源优先走 `AssetManager.openFd` → AppSrc。
    /// - **iOS/macOS**：资源必须解析为磁盘路径，失败即报错（无 AppSrc 回退）。
    /// - **其他桌面**：路径解析失败时回退到 AppSrc。
    pub fn resolve(self) -> Result<ResolvedSource> {
        match self {
            MediaSource::Uri(uri) => Ok(ResolvedSource::Uri(normalize_uri(&uri)?)),
            MediaSource::FlutterAsset(key) => resolve_flutter_asset(&key),
        }
    }
}

/// 将网络 URL 或本地路径转换为 GStreamer URI 字符串。
/// Turns a network URL or local path into a GStreamer URI.
///
/// # 参数 / Parameters
/// - `input` — 原始 URI 或文件路径 / Raw URI or file path.
///
/// # 返回值 / Returns
/// - 规范化后的 URI 字符串（绝对路径会转为 `file://`）/ Normalized URI string
///   (absolute paths become `file://`).
///
/// # 错误 / Errors
/// - 空输入、无效文件路径或无法获取当前工作目录时失败。
///   Fails on empty input, invalid file path, or inability to obtain the current directory.
pub fn normalize_uri(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("empty media URI"));
    }
    if let Ok(parsed) = url::Url::parse(trimmed) {
        if parsed.scheme() == "file" {
            return Ok(parsed.to_string());
        }
        if !parsed.scheme().is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    let path = std::path::Path::new(trimmed);
    if path.is_absolute() || trimmed.starts_with('/') {
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        return Ok(url::Url::from_file_path(&abs)
            .map_err(|_| anyhow!("invalid file path: {}", abs.display()))?
            .to_string());
    }
    Ok(trimmed.to_string())
}

/// 将 Flutter 资源键解析为 [`ResolvedSource`]（平台相关策略）。
/// Resolves a Flutter asset key to a [`ResolvedSource`] (platform-specific strategy).
///
/// # 参数 / Parameters
/// - `asset_key` — Flutter 资源键 / Flutter asset key.
///
/// # 返回值 / Returns
/// - 平台最优的 [`ResolvedSource`] / Platform-optimal [`ResolvedSource`].
///
/// # 错误 / Errors
/// - 空键或资源不可用时失败 / Fails on empty key or unavailable asset.
fn resolve_flutter_asset(asset_key: &str) -> Result<ResolvedSource> {
    let key = asset_key.trim();
    if key.is_empty() {
        return Err(anyhow!("empty asset key"));
    }

    #[cfg(target_os = "android")]
    {
        if asset_android::can_open_asset_fd(key) {
            return Ok(ResolvedSource::AppSrc(key.to_string()));
        }
        return Err(anyhow!(
            "Android asset unavailable: {key} (see logcat FlutterAssetHelper)"
        ));
    }

    #[cfg(not(target_os = "android"))]
    {
        match resolve_flutter_asset_path(key) {
            Ok(path) => {
                let uri = url::Url::from_file_path(&path)
                    .map_err(|_| anyhow!("invalid asset path: {}", path.display()))?
                    .to_string();
                Ok(ResolvedSource::Uri(uri))
            }
            Err(e) => {
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                {
                    return Err(e);
                }
                #[cfg(not(any(target_os = "macos", target_os = "ios")))]
                {
                    log::debug!("asset path resolution failed: {e:#}, falling back to AppSrc");
                    Ok(ResolvedSource::AppSrc(key.to_string()))
                }
            }
        }
    }
}

/// 判断解析后的源是否支持精确 seek。
/// Whether the resolved source supports accurate seeking.
///
/// # 参数 / Parameters
/// - `resolved` — 已解析的媒体源 / Resolved media source.
///
/// # 返回值 / Returns
/// - 仅 [`ResolvedSource::Uri`] 返回 `true`（AppSrc 流式推送通常不可 seek）。
///   Only [`ResolvedSource::Uri`] returns `true` (AppSrc byte push is typically not seekable).
pub fn is_seekable(resolved: &ResolvedSource) -> bool {
    matches!(resolved, ResolvedSource::Uri(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_http_uri_passthrough() {
        let uri = normalize_uri("https://example.com/video.mp4").unwrap();
        assert_eq!(uri, "https://example.com/video.mp4");
    }

    #[test]
    fn normalize_absolute_file_path() {
        let uri = normalize_uri("/tmp/sample.mp4").unwrap();
        assert!(uri.starts_with("file://"));
        assert!(uri.contains("sample.mp4"));
    }

    #[test]
    fn media_source_uri_resolution() {
        let src = MediaSource::Uri("rtsp://host/stream".into())
            .resolve()
            .unwrap();
        assert!(matches!(src, ResolvedSource::Uri(u) if u == "rtsp://host/stream"));
    }
}
