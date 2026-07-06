use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{anyhow, Result};

/// Byte source for AppSrc `need-data` callbacks.
pub enum AssetByteSource {
    File(File),
    #[cfg(target_os = "android")]
    AndroidFd {
        file: File,
        start: u64,
        length: u64,
        position: u64,
    },
}

impl AssetByteSource {
    pub fn open(asset_key: &str) -> Result<Self> {
        #[cfg(target_os = "android")]
        {
            if let Ok(source) = crate::asset_resolver_android::open_asset_fd(asset_key) {
                return Ok(source);
            }
        }
        let path = resolve_flutter_asset_path(asset_key)?;
        let file = File::open(&path)
            .map_err(|e| anyhow!("failed to open asset {}: {e}", path.display()))?;
        Ok(AssetByteSource::File(file))
    }

    pub fn read_chunk(&mut self, max_bytes: usize) -> Result<(Vec<u8>, bool)> {
        let mut buf = vec![0u8; max_bytes];
        let read = match self {
            AssetByteSource::File(file) => file.read(&mut buf)?,
            #[cfg(target_os = "android")]
            AssetByteSource::AndroidFd {
                file,
                start,
                length,
                position,
            } => {
                if *position >= *length {
                    return Ok((Vec::new(), true));
                }
                let remaining = (*length - *position) as usize;
                let to_read = remaining.min(max_bytes);
                file.seek(SeekFrom::Start(*start + *position))?;
                let n = file.read(&mut buf[..to_read])?;
                *position += n as u64;
                n
            }
        };
        buf.truncate(read);
        let eos = match self {
            AssetByteSource::File(_) => read == 0,
            #[cfg(target_os = "android")]
            AssetByteSource::AndroidFd {
                position, length, ..
            } => read == 0 || *position >= *length,
        };
        Ok((buf, eos))
    }
}

/// Resolves a Flutter asset key to an on-disk path under `flutter_assets/`.
pub fn resolve_flutter_asset_path(asset_key: &str) -> Result<PathBuf> {
    let key = asset_key.trim_start_matches('/');
    let candidates = flutter_asset_candidates(key);
    for path in candidates {
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(anyhow!(
        "flutter asset not found: {asset_key} (searched {} paths)",
        flutter_asset_candidates(key).len()
    ))
}

fn flutter_asset_candidates(asset_key: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        #[cfg(target_os = "macos")]
        {
            if let Some(contents) = exe.parent().and_then(|p| p.parent()) {
                out.push(
                    contents
                        .join("Frameworks")
                        .join("App.framework")
                        .join("Resources")
                        .join("flutter_assets")
                        .join(asset_key),
                );
                out.push(
                    contents
                        .join("Frameworks")
                        .join("App.framework")
                        .join("Versions")
                        .join("A")
                        .join("Resources")
                        .join("flutter_assets")
                        .join(asset_key),
                );
            }
        }
        #[cfg(target_os = "windows")]
        {
            if let Some(dir) = exe.parent() {
                out.push(dir.join("data").join("flutter_assets").join(asset_key));
            }
        }
        #[cfg(target_os = "linux")]
        {
            if let Some(dir) = exe.parent() {
                out.push(dir.join("data").join("flutter_assets").join(asset_key));
                out.push(dir.join("flutter_assets").join(asset_key));
            }
        }
    }
    #[cfg(target_os = "ios")]
    {
        out.push(
            PathBuf::from(std::env::var("FLUTTER_ASSETS_DIR").unwrap_or_default()).join(asset_key),
        );
    }
    out
}

/// Shared reader state for AppSrc callbacks.
pub struct AppSrcFeedState {
    pub source: Mutex<AssetByteSource>,
}

impl AppSrcFeedState {
    pub fn new(asset_key: &str) -> Result<Self> {
        Ok(Self {
            source: Mutex::new(AssetByteSource::open(asset_key)?),
        })
    }
}
