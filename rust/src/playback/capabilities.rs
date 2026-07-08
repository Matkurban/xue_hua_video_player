//! 管线能力查询 / Pipeline capability queries.
//!
//! 根据当前 [`super::shell::PipelineShell`] 的源类型（URI playbin vs AppSrc 资产）
//! 向 Dart 层报告 seek、多轨、画面旋转等特性是否可用。
//!
//! Reports seek, multi-track, and orientation support to Dart based on the active
//! [`super::shell::PipelineShell`] source kind (URI playbin vs AppSrc asset).

use super::shell::SourceKind;

/// 当前管线 shell 支持的功能集 / Features available on the active [`super::shell::PipelineShell`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineCapabilities {
    /// 是否支持 seek / Whether seeking is supported.
    pub seek: bool,
    /// 是否支持音轨/字幕选择 / Whether audio/subtitle track selection is supported.
    pub tracks: bool,
    /// 是否支持 videoflip 画面旋转 / Whether videoflip orientation is supported.
    pub orientation: bool,
}

impl PipelineCapabilities {
    /// `playbin3` URI 管线的完整能力 / Full capabilities for `playbin3` URI pipelines.
    pub const PLAYBIN: Self = Self {
        seek: true,
        tracks: true,
        orientation: true,
    };

    /// AppSrc 资产管线的受限能力（无 seek/多轨/旋转）/ Limited capabilities for AppSrc asset pipelines.
    pub const APPSRC: Self = Self {
        seek: false,
        tracks: false,
        orientation: false,
    };

    /// 由 [`SourceKind`] 推导能力 / Derives capabilities from [`SourceKind`].
    ///
    /// # 参数 / Parameters
    /// - `kind` — URI（playbin）或 Asset（AppSrc）/ URI (playbin) or Asset (AppSrc)
    ///
    /// # 返回值 / Returns
    /// - [`PipelineCapabilities::PLAYBIN`] 或 [`PipelineCapabilities::APPSRC`]
    ///
    /// # 错误 / Errors
    /// - 无 / None
    ///
    /// # 线程 / Threading
    /// - 纯函数，任意线程可调用 / Pure function; callable from any thread
    ///
    /// # 平台 / Platform
    /// - 与平台无关 / Platform-independent
    pub fn from_source_kind(kind: SourceKind) -> Self {
        match kind {
            SourceKind::Uri => Self::PLAYBIN,
            SourceKind::Asset => Self::APPSRC,
        }
    }
}
