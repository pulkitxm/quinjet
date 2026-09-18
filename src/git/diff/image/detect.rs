use std::path::Path;

use super::{ImageProtocol, SniffedImage};
use crate::file_icons;

pub(crate) fn sniff_image(bytes: &[u8]) -> Option<SniffedImage> {
    if looks_like_svg(bytes) {
        return Some(SniffedImage::Svg);
    }
    if is_png(bytes)
        || is_jpeg(bytes)
        || is_gif(bytes)
        || is_webp(bytes)
        || is_bmp(bytes)
        || is_ico(bytes)
        || is_tiff(bytes)
        || is_qoi(bytes)
        || is_pnm(bytes)
    {
        return Some(SniffedImage::Raster);
    }
    None
}

pub(crate) fn looks_like_image(path: &Path, bytes: Option<&[u8]>) -> bool {
    file_icons::is_image_path(path) || bytes.is_some_and(|bytes| sniff_image(bytes).is_some())
}

pub(crate) fn is_svg_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

pub(super) fn looks_like_svg(bytes: &[u8]) -> bool {
    let prefix = bytes.get(..bytes.len().min(2048)).unwrap_or_default();
    let Ok(text) = std::str::from_utf8(prefix) else {
        return false;
    };
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    trimmed.starts_with("<svg")
        || (trimmed.starts_with("<?xml") && trimmed.to_ascii_lowercase().contains("<svg"))
}

pub(crate) fn detect_protocol_from_vars<I, K, V>(vars: I) -> ImageProtocol
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut term = String::new();
    let mut term_program = String::new();
    let mut kitty = false;
    let mut iterm_session = false;
    let mut wezterm = false;
    let mut override_protocol = None;
    for (key, value) in vars {
        let key = key.as_ref();
        let value = value.as_ref();
        if value.is_empty() {
            continue;
        }
        match key {
            "TERM" => term = value.to_ascii_lowercase(),
            "TERM_PROGRAM" => term_program = value.to_ascii_lowercase(),
            "KITTY_WINDOW_ID" => kitty = true,
            "ITERM_SESSION_ID" => iterm_session = true,
            "WEZTERM_EXECUTABLE" => wezterm = true,
            "QUINJET_IMAGE_PROTOCOL" => {
                override_protocol = match value.to_ascii_lowercase().as_str() {
                    "kitty" => Some(ImageProtocol::Kitty),
                    "iterm2" => Some(ImageProtocol::Iterm2),
                    "sixel" => Some(ImageProtocol::Sixel),
                    "halfblocks" => Some(ImageProtocol::Halfblocks),
                    _ => None,
                };
            }
            _ => {}
        }
    }
    if let Some(protocol) = override_protocol {
        return protocol;
    }
    if kitty
        || term.contains("kitty")
        || term.contains("ghostty")
        || term_program.contains("ghostty")
    {
        return ImageProtocol::Kitty;
    }
    if iterm_session
        || wezterm
        || term_program.contains("iterm")
        || term_program.contains("wezterm")
        || term_program.contains("mintty")
        || term_program.contains("vscode")
        || term_program.contains("tabby")
        || term_program.contains("hyper")
        || term_program.contains("warp")
    {
        return ImageProtocol::Iterm2;
    }
    if term.contains("sixel") || term.contains("mlterm") || term.contains("foot") {
        return ImageProtocol::Sixel;
    }
    ImageProtocol::Halfblocks
}

fn is_png(bytes: &[u8]) -> bool {
    matches_prefix(bytes, &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
}

fn is_jpeg(bytes: &[u8]) -> bool {
    matches_prefix(bytes, &[0xFF, 0xD8, 0xFF])
}

fn is_gif(bytes: &[u8]) -> bool {
    matches_prefix(bytes, b"GIF87a") || matches_prefix(bytes, b"GIF89a")
}

fn is_webp(bytes: &[u8]) -> bool {
    matches_prefix(bytes, b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
}

fn is_bmp(bytes: &[u8]) -> bool {
    matches_prefix(bytes, b"BM")
}

fn is_ico(bytes: &[u8]) -> bool {
    matches_prefix(bytes, &[0x00, 0x00, 0x01, 0x00])
}

fn is_tiff(bytes: &[u8]) -> bool {
    matches_prefix(bytes, b"II*\0") || matches_prefix(bytes, b"MM\0*")
}

fn is_qoi(bytes: &[u8]) -> bool {
    matches_prefix(bytes, b"qoif")
}

fn is_pnm(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..2),
        Some(b"P1" | b"P2" | b"P3" | b"P4" | b"P5" | b"P6")
    )
}

fn matches_prefix(bytes: &[u8], prefix: &[u8]) -> bool {
    bytes.get(..prefix.len()) == Some(prefix)
}
