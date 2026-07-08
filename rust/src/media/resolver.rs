//! Flutter 资源路径解析与 AppSrc 字节源。
//!
//! Flutter asset path resolution and AppSrc byte sources.
//!
//! 负责记录 Flutter 资源目录、在 Darwin/桌面布局中搜索 `flutter_assets/`，
//! 并为 AppSrc `need-data` 回调提供可重绕的字节读取器。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};

static FLUTTER_ASSETS_DIR: OnceLock<String> = OnceLock::new();

/// 从原生初始化（iOS 插件注册）记录 Flutter 资源目录。
/// Records the Flutter assets directory from native init (iOS plugin register).
///
/// # 参数 / Parameters
/// - `dir` — `flutter_assets` 父目录的绝对路径 / Absolute path to the parent of
///   `flutter_assets`.
///
/// # 返回值 / Returns
/// 无 / None.
///
/// # 线程 / Threading
/// - 应在插件初始化阶段、并发资源加载之前调用 / Should be called during plugin init
///   before concurrent asset loads.
pub fn set_flutter_assets_dir(dir: &str) {
    if dir.is_empty() {
        return;
    }
    let _ = FLUTTER_ASSETS_DIR.set(dir.to_string());
    // SAFETY: called during plugin init before concurrent asset loads.
    unsafe {
        std::env::set_var("FLUTTER_ASSETS_DIR", dir);
    }
    log::info!("flutter assets dir set: {dir}");
}

/// 读取 `OnceLock` 或 `FLUTTER_ASSETS_DIR` 环境变量中的资源目录覆盖值。
/// Reads the assets directory override from `OnceLock` or `FLUTTER_ASSETS_DIR` env var.
fn flutter_assets_dir_override() -> Option<String> {
    FLUTTER_ASSETS_DIR
        .get()
        .cloned()
        .or_else(|| std::env::var("FLUTTER_ASSETS_DIR").ok())
        .filter(|dir| !dir.is_empty())
}

/// AppSrc `need-data` 回调使用的字节源。
/// Byte source for AppSrc `need-data` callbacks.
pub enum AssetByteSource {
    /// 普通磁盘文件 / Regular on-disk file.
    File(File),
    /// Android `AssetManager.openFd` 返回的 fd 区间 / fd range from Android `AssetManager.openFd`.
    #[cfg(target_os = "android")]
    AndroidFd {
        /// 底层文件描述符 / Underlying file descriptor.
        file: File,
        /// 资源在 APK 中的起始字节偏移 / Start byte offset within the APK asset.
        start: u64,
        /// 资源总长度 / Total asset length.
        length: u64,
        /// 当前读取位置（相对 `start`）/ Current read position (relative to `start`).
        position: u64,
    },
}

impl AssetByteSource {
    /// 打开指定 Flutter 资源键的字节源。
    /// Opens a byte source for the given Flutter asset key.
    ///
    /// # 参数 / Parameters
    /// - `asset_key` — Flutter 资源键 / Flutter asset key.
    ///
    /// # 返回值 / Returns
    /// - [`AssetByteSource::File`] 或 Android 上的 [`AssetByteSource::AndroidFd`].
    ///
    /// # 错误 / Errors
    /// - 路径解析失败或文件打开失败时返回错误 / Fails on path resolution or file open errors.
    ///
    /// # 平台 / Platform
    /// - **Android**：优先尝试 `open_asset_fd` / Prefers `open_asset_fd` on Android.
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

    /// 读取最多 `max_bytes` 字节，返回数据与是否到达流末尾。
    /// Reads up to `max_bytes` bytes, returning data and end-of-stream flag.
    ///
    /// # 参数 / Parameters
    /// - `max_bytes` — 单次读取上限 / Maximum bytes to read in one call.
    ///
    /// # 返回值 / Returns
    /// - `(Vec<u8>, bool)`：数据块与 EOS 标志 / Data chunk and EOS flag.
    ///
    /// # 错误 / Errors
    /// - 底层 I/O 或 seek 失败时返回错误 / Fails on underlying I/O or seek errors.
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

    /// 将字节游标回绕到起点，以便 EOS 后 AppSrc 可从头再次推送。
    /// Rewinds the byte cursor so AppSrc can push from the start again after EOS.
    ///
    /// # 参数 / Parameters
    /// 无 / None.
    ///
    /// # 返回值 / Returns
    /// - 成功时返回 `Ok(())` / `Ok(())` on success.
    ///
    /// # 错误 / Errors
    /// - 文件 seek 失败时返回错误 / Fails if file seek fails.
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

/// 将 Flutter 资源键解析为 `flutter_assets/` 下的磁盘路径。
/// Resolves a Flutter asset key to an on-disk path under `flutter_assets/`.
///
/// # 参数 / Parameters
/// - `asset_key` — Flutter 资源键（可带前导 `/`）/ Flutter asset key (optional leading `/`).
///
/// # 返回值 / Returns
/// - 首个存在的候选路径 / First existing candidate path.
///
/// # 错误 / Errors
/// - 所有候选路径均不存在时返回错误（含已搜索路径列表）。
///   Returns an error listing all searched paths when none exist.
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

/// 返回用于测试与诊断的候选路径列表。
/// Returns candidate paths for tests and diagnostics.
///
/// # 参数 / Parameters
/// - `asset_key` — Flutter 资源键 / Flutter asset key.
///
/// # 返回值 / Returns
/// - 按优先级排列的候选绝对路径 / Candidate absolute paths in priority order.
///
/// # 平台 / Platform
/// - 包含 `FLUTTER_ASSETS_DIR` 覆盖、Darwin 框架布局，以及 Windows/Linux 可执行文件旁路径。
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

/// 从 iOS 可执行路径推导 bundle 根目录：`Runner.app/Runner` → `Runner.app`。
/// Derives the iOS bundle root from the executable path: `Runner.app/Runner` → `Runner.app`.
///
/// # 参数 / Parameters
/// - `exe` — 可执行文件路径 / Executable file path.
///
/// # 返回值 / Returns
/// - bundle 根目录，无法推导时返回 `None` / Bundle root, or `None` if not derivable.
pub(crate) fn ios_bundle_root(exe: &Path) -> Option<PathBuf> {
    exe.parent().map(|path| path.to_path_buf())
}

/// 从 macOS 可执行路径推导 Contents 根目录：`…/Contents/MacOS/Runner` → `…/Contents`。
/// Derives the macOS Contents root: `…/Contents/MacOS/Runner` → `…/Contents`.
///
/// # 参数 / Parameters
/// - `exe` — 可执行文件路径 / Executable file path.
///
/// # 返回值 / Returns
/// - `Contents` 目录，无法推导时返回 `None` / `Contents` directory, or `None`.
pub(crate) fn macos_bundle_contents_root(exe: &Path) -> Option<PathBuf> {
    exe.parent()
        .and_then(|macos| macos.parent())
        .map(|path| path.to_path_buf())
}

/// 根据目标 OS 从可执行路径推导 Darwin bundle 根目录。
/// Derives the Darwin bundle root from the executable path for the target OS.
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

/// 在 Darwin bundle 根下生成框架相对 `flutter_assets/` 候选路径。
/// Framework-relative `flutter_assets/` candidates under a Darwin bundle root.
///
/// # 参数 / Parameters
/// - `bundle_root` — iOS `.app` 或 macOS `Contents` 根目录 / iOS `.app` or macOS `Contents` root.
/// - `asset_key` — Flutter 资源键 / Flutter asset key.
///
/// # 返回值 / Returns
/// - `App.framework` 与 `Flutter.framework` 的多种布局候选（共 5 条）/ Five layout variants
///   under `App.framework` and `Flutter.framework`.
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

/// 从当前可执行文件路径生成 Darwin 平台的资源候选路径。
/// Generates Darwin asset candidate paths from the current executable path.
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

/// AppSrc 回调共享的读取器状态（线程安全）。
/// Shared reader state for AppSrc callbacks (thread-safe).
pub struct AppSrcFeedState {
    /// 受互斥锁保护的字节源 / Mutex-protected byte source.
    pub source: Mutex<AssetByteSource>,
}

impl AppSrcFeedState {
    /// 为指定资源键创建新的 AppSrc 喂送状态。
    /// Creates new AppSrc feed state for the given asset key.
    ///
    /// # 参数 / Parameters
    /// - `asset_key` — Flutter 资源键 / Flutter asset key.
    ///
    /// # 返回值 / Returns
    /// - 已打开字节源的 [`AppSrcFeedState`] / [`AppSrcFeedState`] with an opened byte source.
    ///
    /// # 错误 / Errors
    /// - [`AssetByteSource::open`] 失败时传播错误 / Propagates [`AssetByteSource::open`] errors.
    pub fn new(asset_key: &str) -> Result<Self> {
        Ok(Self {
            source: Mutex::new(AssetByteSource::open(asset_key)?),
        })
    }

    /// 将内部字节源回绕到起点（用于循环或重新播放）。
    /// Rewinds the internal byte source to the start (for looping or replay).
    ///
    /// # 参数 / Parameters
    /// 无 / None.
    ///
    /// # 返回值 / Returns
    /// - 成功时返回 `Ok(())` / `Ok(())` on success.
    ///
    /// # 错误 / Errors
    /// - 互斥锁中毒或底层 rewind 失败时返回错误 / Fails on poisoned mutex or rewind errors.
    pub fn rewind(&self) -> Result<()> {
        self.source
            .lock()
            .map_err(|_| anyhow!("AppSrc feed lock poisoned"))?
            .rewind()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn asset_byte_source_rewind_after_eof() {
        let path = std::env::temp_dir().join("xhvp_rewind_test.bin");
        {
            let mut file = File::create(&path).unwrap();
            file.write_all(b"hello world").unwrap();
        }
        let mut source = AssetByteSource::File(File::open(&path).unwrap());
        let (chunk, eos) = source.read_chunk(1024).unwrap();
        assert_eq!(chunk, b"hello world");
        assert!(!eos);

        let (chunk2, eos2) = source.read_chunk(1024).unwrap();
        assert!(chunk2.is_empty());
        assert!(eos2);

        source.rewind().unwrap();
        let (chunk3, eos3) = source.read_chunk(1024).unwrap();
        assert_eq!(chunk3, b"hello world");
        assert!(!eos3);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ios_bundle_root_from_runner_exe() {
        let exe = PathBuf::from("/var/containers/Bundle/Application/ABC/Runner.app/Runner");
        let root = ios_bundle_root(&exe).unwrap();
        assert_eq!(
            root,
            PathBuf::from("/var/containers/Bundle/Application/ABC/Runner.app")
        );
    }

    #[test]
    fn macos_bundle_root_from_runner_exe() {
        let exe = PathBuf::from("/Applications/MyApp.app/Contents/MacOS/Runner");
        let root = macos_bundle_contents_root(&exe).unwrap();
        assert_eq!(root, PathBuf::from("/Applications/MyApp.app/Contents"));
    }

    #[test]
    fn darwin_framework_candidates_ios_layout() {
        let bundle = PathBuf::from("/var/containers/Bundle/Application/ABC/Runner.app");
        let key = "assets/sample.mp4";
        let candidates = darwin_framework_asset_candidates(&bundle, key);
        assert_eq!(candidates.len(), 5);
        assert_eq!(
            candidates[0],
            bundle
                .join("Frameworks")
                .join("App.framework")
                .join("flutter_assets")
                .join(key)
        );
    }

    #[test]
    fn darwin_framework_candidates_macos_layout() {
        let bundle = PathBuf::from("/Applications/MyApp.app/Contents");
        let key = "assets/sample.mp4";
        let candidates = darwin_framework_asset_candidates(&bundle, key);
        assert_eq!(candidates.len(), 5);
        assert!(
            candidates[0].ends_with("Frameworks/App.framework/flutter_assets/assets/sample.mp4")
        );
    }

    #[test]
    #[cfg(target_os = "ios")]
    fn darwin_candidates_for_ios_exe_path() {
        let exe = PathBuf::from("/var/containers/Bundle/Application/ABC/Runner.app/Runner");
        let key = "assets/sample.mp4";
        let candidates = darwin_candidates_for_exe(&exe, key);
        assert_eq!(candidates.len(), 5);
        assert!(candidates[0]
            .ends_with("Runner.app/Frameworks/App.framework/flutter_assets/assets/sample.mp4"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn darwin_candidates_for_macos_exe_path() {
        let exe = PathBuf::from("/Applications/MyApp.app/Contents/MacOS/Runner");
        let key = "assets/sample.mp4";
        let candidates = darwin_candidates_for_exe(&exe, key);
        assert_eq!(candidates.len(), 5);
        assert!(candidates[0]
            .ends_with("Contents/Frameworks/App.framework/flutter_assets/assets/sample.mp4"));
    }
}
