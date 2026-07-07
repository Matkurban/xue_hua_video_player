#[cfg(target_os = "android")]
mod asset_android;
mod resolver;

pub use resolver::{resolve_flutter_asset_path, AppSrcFeedState};

use anyhow::{anyhow, Result};

/// Media input accepted by [`crate::playback::PlaybackEngine::load`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaSource {
    /// `http(s)://`, `rtsp://`, `file://`, HLS, etc.
    Uri(String),
    /// Flutter asset key, e.g. `assets/sample.mp4`.
    FlutterAsset(String),
}

/// How a [`MediaSource`] is wired into GStreamer after resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSource {
    Uri(String),
    AppSrc(String),
}

impl MediaSource {
    /// Normalizes network/file URIs and resolves Flutter assets to the best playback path.
    pub fn resolve(self) -> Result<ResolvedSource> {
        match self {
            MediaSource::Uri(uri) => Ok(ResolvedSource::Uri(normalize_uri(&uri)?)),
            MediaSource::FlutterAsset(key) => resolve_flutter_asset(&key),
        }
    }
}

/// Turns a network URL or local path into a GStreamer URI.
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
        if let Ok(path) = resolve_flutter_asset_path(key) {
            let uri = url::Url::from_file_path(&path)
                .map_err(|_| anyhow!("invalid asset path: {}", path.display()))?
                .to_string();
            return Ok(ResolvedSource::Uri(uri));
        }

        Ok(ResolvedSource::AppSrc(key.to_string()))
    }
}

/// Whether the resolved source supports accurate seeking.
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
