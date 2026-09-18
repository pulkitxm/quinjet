use std::sync::{Arc, OnceLock};

use serde::Serialize;

mod attach;
mod decode;
mod detect;
mod source;
#[cfg(test)]
mod tests;

pub(crate) use attach::attach_image_previews;
pub(crate) use detect::detect_protocol_from_vars;
pub(crate) use source::{BlobOrigin, LoadedBlob, RevisionImageSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ImageSide {
    Previous,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ImagePreviewKind {
    Raster,
    Svg,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ImageProtocol {
    Halfblocks,
    Sixel,
    Kitty,
    Iterm2,
}

impl ImageProtocol {
    pub(crate) fn detect() -> Self {
        static PROTOCOL: OnceLock<ImageProtocol> = OnceLock::new();
        *PROTOCOL.get_or_init(|| detect_protocol_from_vars(std::env::vars()))
    }

    pub(crate) const fn is_native(self) -> bool {
        !matches!(self, Self::Halfblocks)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImageCell {
    pub upper: [u8; 3],
    pub lower: [u8; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageRaster {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImagePreview {
    pub side: ImageSide,
    pub path: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub kind: ImagePreviewKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub row: u16,
    pub rows: u16,
    #[serde(skip)]
    pub cells: Vec<ImageCell>,
    #[serde(skip)]
    pub raster: Option<Arc<ImageRaster>>,
}

impl ImagePreview {
    pub(crate) fn caption(&self) -> String {
        let side = match self.side {
            ImageSide::Previous => "previous",
            ImageSide::New => "new",
        };
        match (self.width, self.height) {
            (Some(width), Some(height)) => format!("image: {width}x{height} {side}"),
            _ => match self.kind {
                ImagePreviewKind::Svg => format!("image: svg {side}"),
                ImagePreviewKind::Skipped => self.reason.as_ref().map_or_else(
                    || format!("image: {side}"),
                    |reason| format!("image: {side} ({reason})"),
                ),
                ImagePreviewKind::Raster => format!("image: {side}"),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SniffedImage {
    Raster,
    Svg,
}
