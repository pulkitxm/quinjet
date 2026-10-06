use std::sync::Arc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::preparation::EncodedImage;
use super::{
    IMAGE_PREPARATION, ImageDrawState, ImageKey, PreparationFrame, Rect, draw_image_line,
    draw_native, prepare_native,
};
use crate::git::diff::{
    DiffLine, DiffLineKind, HighlightSpan, ImageCell, ImagePreview, ImagePreviewKind,
    ImageProtocol, ImageRaster, ImageSide,
};
use crate::theme::{Appearance, Theme, ThemeName};

#[test]
fn native_protocols_write_image_payloads_for_both_sides() {
    for (protocol, marker) in [
        (ImageProtocol::Kitty, "\x1b_G"),
        (ImageProtocol::Iterm2, "]1337;File=inline=1"),
        (ImageProtocol::Sixel, "\x1bP"),
    ] {
        let backend = TestBackend::new(12, 4);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let previous = preview(ImageSide::Previous);
        let current = preview(ImageSide::New);
        warm_native_images(&[&previous, &current], protocol, 5);
        let _frame = terminal
            .draw(|frame| {
                let _images = PreparationFrame::begin();
                assert!(draw_native(
                    frame,
                    Rect::new(0, 0, 5, 2),
                    &previous,
                    protocol,
                    2
                ));
                assert!(draw_native(
                    frame,
                    Rect::new(6, 0, 5, 2),
                    &current,
                    protocol,
                    2
                ));
            })
            .expect("native frame");
        let symbols = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(symbols.matches(marker).count() >= 2, "{protocol:?}");
    }
}

#[test]
fn scrolled_away_native_images_leave_no_stale_cells() {
    for protocol in [
        ImageProtocol::Kitty,
        ImageProtocol::Iterm2,
        ImageProtocol::Sixel,
    ] {
        let mut terminal = Terminal::new(TestBackend::new(12, 4)).expect("test terminal");
        let previous = preview(ImageSide::Previous);
        let current = preview(ImageSide::New);
        warm_native_images(&[&previous, &current], protocol, 5);
        let _frame = terminal
            .draw(|frame| {
                let _images = PreparationFrame::begin();
                assert!(draw_native(
                    frame,
                    Rect::new(0, 0, 5, 2),
                    &previous,
                    protocol,
                    2
                ));
                assert!(draw_native(
                    frame,
                    Rect::new(6, 0, 5, 2),
                    &current,
                    protocol,
                    2
                ));
            })
            .expect("native frame");
        let _frame = terminal.draw(|_| {}).expect("empty frame");
        assert!(
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .all(|cell| cell.symbol() == " "),
            "{protocol:?}"
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the test follows one image pair through repeated scroll positions"
)]
fn scrolling_keeps_native_pair_dimensions_and_clears_stale_placements() {
    let theme = Theme::new(ThemeName::Quinjet, Appearance::Dark);
    let mut lake = preview(ImageSide::New);
    lake.rows = 8;
    lake.raster = Some(Arc::new(ImageRaster {
        width: 2,
        height: 64,
        rgba: vec![255; 512],
    }));
    let mut previous = preview(ImageSide::Previous);
    previous.rows = 8;
    previous.raster = Some(Arc::new(ImageRaster {
        width: 2,
        height: 64,
        rgba: vec![255; 512],
    }));
    let mut current = preview(ImageSide::New);
    current.rows = 8;
    current.raster = Some(Arc::new(ImageRaster {
        width: 2,
        height: 64,
        rgba: vec![255; 512],
    }));
    warm_native_images(&[&lake, &previous, &current], ImageProtocol::Kitty, 5);
    let lake_line = image_line(lake);
    let previous_line = image_line(previous.clone());
    let current_line = image_line(current.clone());
    let mut terminal = Terminal::new(TestBackend::new(24, 16)).expect("scroll viewport");
    let mut third_row = None;

    for pass in 0..12 {
        let _frame = terminal
            .draw(|frame| {
                let _images = PreparationFrame::begin();
                let mut state = ImageDrawState::default();
                match pass % 4 {
                    0 => {
                        draw_image_line(
                            frame,
                            Rect::new(12, 0, 5, 1),
                            &lake_line,
                            16,
                            &theme,
                            ImageProtocol::Kitty,
                            &mut state,
                        );
                        draw_image_line(
                            frame,
                            Rect::new(0, 13, 5, 1),
                            &previous_line,
                            3,
                            &theme,
                            ImageProtocol::Kitty,
                            &mut state,
                        );
                        draw_image_line(
                            frame,
                            Rect::new(12, 13, 5, 1),
                            &current_line,
                            3,
                            &theme,
                            ImageProtocol::Kitty,
                            &mut state,
                        );
                    }
                    1 => {
                        draw_image_line(
                            frame,
                            Rect::new(0, 0, 5, 1),
                            &previous_line,
                            16,
                            &theme,
                            ImageProtocol::Kitty,
                            &mut state,
                        );
                        draw_image_line(
                            frame,
                            Rect::new(12, 0, 5, 1),
                            &current_line,
                            16,
                            &theme,
                            ImageProtocol::Kitty,
                            &mut state,
                        );
                    }
                    2 => {
                        let mut clipped_previous = previous.clone();
                        clipped_previous.row = 3;
                        let mut clipped_current = current.clone();
                        clipped_current.row = 3;
                        draw_image_line(
                            frame,
                            Rect::new(0, 0, 5, 1),
                            &image_line(clipped_previous),
                            16,
                            &theme,
                            ImageProtocol::Kitty,
                            &mut state,
                        );
                        draw_image_line(
                            frame,
                            Rect::new(12, 0, 5, 1),
                            &image_line(clipped_current),
                            16,
                            &theme,
                            ImageProtocol::Kitty,
                            &mut state,
                        );
                    }
                    _ => {}
                }
            })
            .expect("scroll frame");
        let buffer = terminal.backend().buffer();
        if pass % 4 == 3 {
            assert!(buffer.content().iter().all(|cell| cell.symbol() == " "));
        } else {
            let y = if pass % 4 == 0 { 13 } else { 0 };
            for x in [0, 12] {
                let symbol = buffer.cell((x, y)).expect("image cell").symbol();
                assert!(
                    symbol.contains('\u{10EEEE}'),
                    "pass {pass}, x {x}, symbol {symbol:?}"
                );
                assert!(
                    !symbol.contains('▀'),
                    "pass {pass}, x {x}, symbol {symbol:?}"
                );
                assert!(
                    symbol.contains("\x1b[s"),
                    "pass {pass}, x {x}, symbol {symbol:?}"
                );
            }
            if pass % 4 == 2 {
                let clipped = buffer.cell((0, 0)).expect("clipped image row").symbol();
                let (_, clipped_row) = clipped.split_once("\x1b[s").expect("Kitty row");
                let (clipped_row, _) = clipped_row.split_once("\x1b[u").expect("row end");
                assert_eq!(Some(clipped_row), third_row.as_deref());
                assert!(!clipped.contains("\x1b_G"));
                assert_eq!(buffer.cell((0, 5)).expect("last image row").symbol(), " ");
            }
            if pass % 4 == 1 {
                let visible = buffer.cell((0, 3)).expect("third image row").symbol();
                let (_, visible) = visible.split_once("\x1b[s").expect("Kitty row");
                let (visible, _) = visible.split_once("\x1b[u").expect("row end");
                third_row = Some(visible.to_owned());
            }
        }
        IMAGE_PREPARATION.with(|preparation| {
            let preparation = preparation.borrow();
            let cache = &preparation.encoded;
            assert!(cache.len() <= 3);
            assert!(
                cache
                    .iter()
                    .all(|entry| entry.key.height == 8 && entry.key.width == 5)
            );
        });
    }
    IMAGE_PREPARATION.with(|preparation| assert_eq!(preparation.borrow().encoded.len(), 3));
}

#[test]
fn other_native_protocols_use_consistent_fallback_when_scrolling() {
    let theme = Theme::new(ThemeName::Quinjet, Appearance::Dark);
    let mut image = preview(ImageSide::New);
    image.rows = 8;
    let mut terminal = Terminal::new(TestBackend::new(12, 10)).expect("scroll viewport");
    for protocol in [ImageProtocol::Iterm2, ImageProtocol::Sixel] {
        warm_native_images(&[&image], protocol, 5);
        for (row, remaining_height, allow_native, expected_native) in [
            (0, 8, true, true),
            (0, 4, true, false),
            (3, 8, true, false),
            (0, 8, false, false),
        ] {
            image.row = row;
            let _frame = terminal
                .draw(|frame| {
                    let _images = PreparationFrame::begin();
                    draw_image_line(
                        frame,
                        Rect::new(0, 0, 5, 1),
                        &image_line(image.clone()),
                        remaining_height,
                        &theme,
                        protocol,
                        &mut ImageDrawState::new(allow_native),
                    );
                })
                .expect("scroll frame");
            assert_eq!(
                terminal
                    .backend()
                    .buffer()
                    .cell((0, 0))
                    .is_some_and(|cell| cell.symbol().contains('\x1b')),
                expected_native,
                "{protocol:?}, row {row}, remaining height {remaining_height}, native {allow_native}"
            );
        }
    }
}

pub(super) fn image_line(preview: ImagePreview) -> DiffLine {
    DiffLine {
        kind: DiffLineKind::Image,
        old_line: None,
        new_line: None,
        spans: vec![HighlightSpan {
            text: preview.caption(),
            foreground: None,
            bold: false,
            italic: false,
        }],
        image: Some(preview),
    }
}

pub(super) fn preview(side: ImageSide) -> ImagePreview {
    ImagePreview {
        side,
        path: "photo.png".to_owned(),
        width: Some(2),
        height: Some(2),
        kind: ImagePreviewKind::Raster,
        reason: None,
        row: 0,
        rows: 2,
        cells: vec![
            ImageCell {
                upper: [255, 0, 0],
                lower: [0, 0, 255],
            };
            5
        ],
        raster: Some(Arc::new(ImageRaster {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
        })),
    }
}

fn warm_native_images(previews: &[&ImagePreview], protocol: ImageProtocol, width: u16) {
    IMAGE_PREPARATION.with(|preparation| {
        let mut preparation = preparation.borrow_mut();
        *preparation = super::preparation::ImagePreparation::default();
        for preview in previews {
            let key = ImageKey {
                raster: Arc::clone(preview.raster.as_ref().expect("test raster")),
                protocol,
                width,
                height: preview.rows,
            };
            let display = prepare_native(&key).expect("prepared test image");
            preparation.encoded.push(EncodedImage {
                key,
                display: Some(display),
            });
        }
    });
}
