use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use image::imageops::FilterType;
use image::{DynamicImage, ImageReader, Limits, RgbaImage};

use super::detect::{is_svg_path, looks_like_svg, sniff_image};
use super::{ImageCell, ImagePreview, ImagePreviewKind, ImageRaster, ImageSide, SniffedImage};
pub(crate) const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_DECODE_DIM: u32 = 4096;
const MAX_RASTER_WIDTH: u32 = 1600;
const MAX_RASTER_HEIGHT: u32 = 1200;
const MAX_CELL_WIDTH: u32 = 96;
const MAX_CELL_PIXEL_HEIGHT: u32 = 64;

pub(crate) fn decode_image(bytes: &[u8], path: &Path, side: ImageSide) -> Vec<ImagePreview> {
    if is_svg_path(path) || sniff_image(bytes) == Some(SniffedImage::Svg) || looks_like_svg(bytes) {
        return vec![fallback(
            path,
            side,
            ImagePreviewKind::Svg,
            Some("vector preview unavailable"),
        )];
    }
    match rasterize(bytes) {
        Some((width, height, raster, cells)) => rows(path, side, width, height, &raster, cells),
        None => vec![fallback(
            path,
            side,
            ImagePreviewKind::Skipped,
            Some("could not decode image"),
        )],
    }
}

pub(crate) fn skipped_too_large(
    path: &Path,
    side: ImageSide,
    byte_count: usize,
) -> Vec<ImagePreview> {
    let kib = byte_count.div_ceil(1024);
    let reason = format!("too large to preview ({kib} KiB)");
    vec![fallback(
        path,
        side,
        ImagePreviewKind::Skipped,
        Some(reason.as_str()),
    )]
}

pub(crate) fn fallback(
    path: &Path,
    side: ImageSide,
    kind: ImagePreviewKind,
    reason: Option<&str>,
) -> ImagePreview {
    ImagePreview {
        side,
        path: path.display().to_string(),
        width: None,
        height: None,
        kind,
        reason: reason.map(ToOwned::to_owned),
        row: 0,
        rows: 1,
        cells: Vec::new(),
        raster: None,
    }
}

type Rasterized = (u32, u32, Arc<ImageRaster>, Vec<Vec<ImageCell>>);

fn rasterize(bytes: &[u8]) -> Option<Rasterized> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODE_DIM);
    limits.max_image_height = Some(MAX_DECODE_DIM);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().ok()?;
    let width = decoded.width();
    let height = decoded.height();
    if width == 0 || height == 0 {
        return None;
    }
    let cells_image = downscale(&decoded, MAX_CELL_WIDTH, MAX_CELL_PIXEL_HEIGHT);
    let cells = halfblock_rows(&cells_image);
    let raster = downscale(&decoded, MAX_RASTER_WIDTH, MAX_RASTER_HEIGHT);
    let stored = Arc::new(ImageRaster {
        width: raster.width(),
        height: raster.height(),
        rgba: raster.into_raw(),
    });
    Some((width, height, stored, cells))
}

fn downscale(image: &DynamicImage, max_width: u32, max_height: u32) -> RgbaImage {
    if image.width() <= max_width && image.height() <= max_height {
        return image.to_rgba8();
    }
    image
        .resize(max_width, max_height, FilterType::Triangle)
        .to_rgba8()
}

fn halfblock_rows(image: &RgbaImage) -> Vec<Vec<ImageCell>> {
    let height = image.height();
    let width = image.width();
    let rows = height.div_ceil(2);
    let mut output = Vec::with_capacity(usize::try_from(rows).unwrap_or_default());
    for row in 0..rows {
        let upper_y = row.saturating_mul(2);
        let lower_y = upper_y.saturating_add(1);
        let mut cells = Vec::with_capacity(usize::try_from(width).unwrap_or_default());
        for x in 0..width {
            cells.push(ImageCell {
                upper: pixel_rgb(image, x, upper_y),
                lower: pixel_rgb(image, x, lower_y),
            });
        }
        output.push(cells);
    }
    output
}

fn pixel_rgb(image: &RgbaImage, x: u32, y: u32) -> [u8; 3] {
    if y >= image.height() {
        return [0, 0, 0];
    }
    blend_on_dark(image.get_pixel(x, y).0)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "alpha blend stays in 0..=255"
)]
#[expect(
    clippy::integer_division,
    reason = "alpha blending uses 8-bit integer maths"
)]
fn blend_on_dark(rgba: [u8; 4]) -> [u8; 3] {
    let [red, green, blue, alpha] = rgba;
    let alpha = u16::from(alpha);
    [
        (u16::from(red) * alpha / 255) as u8,
        (u16::from(green) * alpha / 255) as u8,
        (u16::from(blue) * alpha / 255) as u8,
    ]
}

fn rows(
    path: &Path,
    side: ImageSide,
    width: u32,
    height: u32,
    raster: &Arc<ImageRaster>,
    cells: Vec<Vec<ImageCell>>,
) -> Vec<ImagePreview> {
    let rows = crate::convert::cells(cells.len());
    if rows == 0 {
        return vec![fallback(
            path,
            side,
            ImagePreviewKind::Skipped,
            Some("empty image"),
        )];
    }
    cells
        .into_iter()
        .enumerate()
        .map(|(index, row_cells)| ImagePreview {
            side,
            path: path.display().to_string(),
            width: Some(width),
            height: Some(height),
            kind: ImagePreviewKind::Raster,
            reason: None,
            row: crate::convert::cells(index),
            rows,
            cells: row_cells,
            raster: Some(Arc::clone(raster)),
        })
        .collect()
}
