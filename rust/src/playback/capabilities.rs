use super::shell::SourceKind;

/// Features available on the active [`super::shell::PipelineShell`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineCapabilities {
    pub seek: bool,
    pub tracks: bool,
    pub orientation: bool,
}

impl PipelineCapabilities {
    pub const PLAYBIN: Self = Self {
        seek: true,
        tracks: true,
        orientation: true,
    };

    pub const APPSRC: Self = Self {
        seek: false,
        tracks: false,
        orientation: false,
    };

    pub fn from_source_kind(kind: SourceKind) -> Self {
        match kind {
            SourceKind::Uri => Self::PLAYBIN,
            SourceKind::Asset => Self::APPSRC,
        }
    }
}
