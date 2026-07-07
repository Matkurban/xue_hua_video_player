use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};

static FLUTTER_ASSETS_DIR: OnceLock<String> = OnceLock::new();

fn flutter_assets_dir_override() -> Option<String> {
    FLUTTER_ASSETS_DIR
        .get()
        .cloned()
        .or_else(|| std::env::var("FLUTTER_ASSETS_DIR").ok())
        .filter(|dir| !dir.is_empty())
}

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
            if let Ok(source) = super::asset_android::open_asset_fd(asset_key) {
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

    /// Rewinds the byte cursor so AppSrc can push from the start again after EOS.
    pub fn rewind(&mut self) -> Result<()> {
        match self {
            AssetByteSource::File(file) => {
                file.seek(SeekFrom::Start(0))?;
            }
            #[cfg(target_os = "android")]
            AssetByteSource::AndroidFd { position, .. } => {
                *position = 0;
            }
        }
        Ok(())
    }
}

/// Resolves a Flutter asset key to an on-disk path under `flutter_assets/`.
pub fn resolve_flutter_asset_path(asset_key: &str) -> Result<PathBuf> {
    let key = asset_key.trim_start_matches('/');
    let candidates = flutter_asset_candidates(key);
    for path in &candidates {
        if path.is_file() {
            return Ok(path.clone());
        }
    }
    let searched: Vec<String> = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    Err(anyhow!(
        "flutter asset not found: {asset_key} (searched {} paths: {})",
        searched.len(),
        searched.join(", ")
    ))
}

/// Returns candidate paths for tests and diagnostics.
pub fn flutter_asset_candidates(asset_key: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();

    if let Some(dir) = flutter_assets_dir_override() {
        out.push(PathBuf::from(dir).join(asset_key));
    }

    if let Ok(exe) = std::env::current_exe() {
        out.extend(darwin_candidates_for_exe(&exe, asset_key));
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

    out
}

/// iOS: `Runner.app/Runner` → `Runner.app`.
pub(crate) fn ios_bundle_root(exe: &Path) -> Option<PathBuf> {
    exe.parent().map(|path| path.to_path_buf())
}

/// macOS: `…/Contents/MacOS/Runner` → `…/Contents`.
pub(crate) fn macos_bundle_contents_root(exe: &Path) -> Option<PathBuf> {
    exe.parent()
        .and_then(|macos| macos.parent())
        .map(|path| path.to_path_buf())
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn darwin_bundle_root(exe: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "ios")]
    {
        ios_bundle_root(exe)
    }
    #[cfg(target_os = "macos")]
    {
        macos_bundle_contents_root(exe)
    }
}

/// Framework-relative `flutter_assets/` candidates under a Darwin bundle root.
pub(crate) fn darwin_framework_asset_candidates(
    bundle_root: &Path,
    asset_key: &str,
) -> Vec<PathBuf> {
    let frameworks = bundle_root.join("Frameworks");
    let mut out = vec![frameworks
        .join("App.framework")
        .join("flutter_assets")
        .join(asset_key)];
    for framework in ["App.framework", "Flutter.framework"] {
        out.push(
            frameworks
                .join(framework)
                .join("Resources")
                .join("flutter_assets")
                .join(asset_key),
        );
        out.push(
            frameworks
                .join(framework)
                .join("Versions")
                .join("A")
                .join("Resources")
                .join("flutter_assets")
                .join(asset_key),
        );
    }
    out
}

fn darwin_candidates_for_exe(exe: &Path, asset_key: &str) -> Vec<PathBuf> {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        if let Some(bundle_root) = darwin_bundle_root(exe) {
            return darwin_framework_asset_candidates(&bundle_root, asset_key);
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let _ = (exe, asset_key);
    }
    Vec::new()
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
