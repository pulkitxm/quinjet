use std::io::Cursor;
use std::path::PathBuf;

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

use super::attach_image_previews;
use super::decode::decode_image;
use super::detect::{detect_protocol_from_vars, sniff_image};
use super::source::MapImageSource;
use super::{ImageProtocol, ImageSide, SniffedImage};
use crate::git::diff::{DiffLineKind, parse_diff};

fn png(color: [u8; 4]) -> Vec<u8> {
    let image = RgbaImage::from_pixel(2, 2, Rgba(color));
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("generated PNG must encode");
    bytes.into_inner()
}

fn preview_sides(patch: &str, source: &MapImageSource) -> Vec<ImageSide> {
    let mut document = parse_diff(patch.as_bytes(), "images", None, false);
    attach_image_previews(&mut document, source);
    document
        .lines
        .iter()
        .filter(|line| line.kind == DiffLineKind::Image)
        .filter_map(|line| line.image.as_ref())
        .filter(|preview| preview.row == 0)
        .map(|preview| preview.side)
        .collect()
}

#[test]
fn image_magic_and_terminal_detection() {
    assert_eq!(
        sniff_image(&png([255, 0, 0, 255])),
        Some(SniffedImage::Raster)
    );
    assert_eq!(sniff_image(b"plain text"), None);
    assert_eq!(
        detect_protocol_from_vars(std::iter::once(("TERM", "xterm-kitty"))),
        ImageProtocol::Kitty
    );
    assert_eq!(
        detect_protocol_from_vars(std::iter::once(("TERM_PROGRAM", "iTerm.app"))),
        ImageProtocol::Iterm2
    );
    assert_eq!(
        detect_protocol_from_vars(std::iter::once(("TERM", "xterm-sixel"))),
        ImageProtocol::Sixel
    );
    assert_eq!(
        detect_protocol_from_vars(std::iter::once(("TERM", "xterm-256color"))),
        ImageProtocol::Halfblocks
    );
    assert_eq!(
        detect_protocol_from_vars(std::iter::once(("TERM_PROGRAM", "ghostty"))),
        ImageProtocol::Kitty
    );
    assert_eq!(
        detect_protocol_from_vars([
            ("TERM", "xterm-kitty"),
            ("QUINJET_IMAGE_PROTOCOL", "halfblocks")
        ]),
        ImageProtocol::Halfblocks
    );
}

#[test]
fn native_raster_retains_photo_pixels_before_terminal_fit() {
    let image = RgbaImage::from_pixel(240, 160, Rgba([255, 120, 40, 255]));
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("generated PNG must encode");
    let previews = decode_image(
        &bytes.into_inner(),
        PathBuf::from("photo.png").as_path(),
        ImageSide::New,
    );
    let raster = previews.first().and_then(|preview| preview.raster.as_ref());
    assert_eq!(
        raster.map(|raster| (raster.width, raster.height)),
        Some((240, 160))
    );
    assert_eq!(previews.len(), 32);
    assert_eq!(
        previews.first().map(|preview| preview.cells.len()),
        Some(96)
    );
}

#[test]
fn modified_added_and_deleted_images_use_available_sides() {
    let path = PathBuf::from("picture.png");
    let mut source = MapImageSource::default();
    drop(source.previous.insert(path.clone(), png([255, 0, 0, 255])));
    drop(source.current.insert(path, png([0, 0, 255, 255])));
    let modified = "diff --git a/picture.png b/picture.png\nBinary files a/picture.png and b/picture.png differ\n";
    assert_eq!(
        preview_sides(modified, &source),
        vec![ImageSide::Previous, ImageSide::New]
    );
    let added = "diff --git a/picture.png b/picture.png\nnew file mode 100644\nBinary files /dev/null and b/picture.png differ\n";
    assert_eq!(preview_sides(added, &source), vec![ImageSide::New]);
    let deleted = "diff --git a/picture.png b/picture.png\ndeleted file mode 100644\nBinary files a/picture.png and /dev/null differ\n";
    assert_eq!(preview_sides(deleted, &source), vec![ImageSide::Previous]);
}

#[test]
fn changed_rename_uses_previous_path_and_new_path() {
    let mut source = MapImageSource::default();
    drop(
        source
            .previous
            .insert(PathBuf::from("before.png"), png([255, 0, 0, 255])),
    );
    drop(
        source
            .current
            .insert(PathBuf::from("after.png"), png([0, 255, 0, 255])),
    );
    let renamed = "diff --git a/before.png b/after.png\nsimilarity index 50%\nrename from before.png\nrename to after.png\nBinary files a/before.png and b/after.png differ\n";
    assert_eq!(
        preview_sides(renamed, &source),
        vec![ImageSide::Previous, ImageSide::New]
    );
}

#[test]
fn non_images_remain_binary_and_large_images_show_a_reason() {
    let mut source = MapImageSource::default();
    drop(
        source
            .current
            .insert(PathBuf::from("binary.dat"), b"binary".to_vec()),
    );
    drop(source.current.insert(
        PathBuf::from("big.png"),
        vec![0; super::decode::MAX_IMAGE_BYTES + 1],
    ));
    let binary = "diff --git a/binary.dat b/binary.dat\nnew file mode 100644\nBinary files /dev/null and b/binary.dat differ\n";
    assert!(preview_sides(binary, &source).is_empty());
    let large = "diff --git a/big.png b/big.png\nnew file mode 100644\nBinary files /dev/null and b/big.png differ\n";
    let mut document = parse_diff(large.as_bytes(), "images", None, false);
    attach_image_previews(&mut document, &source);
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.text().contains("too large to preview"))
    );
}
