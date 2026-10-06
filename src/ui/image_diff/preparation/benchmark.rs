use std::io::Write;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;
use crate::git::diff::{ImageCell, ImageSide};
use crate::theme::{Appearance, Theme, ThemeName};
use crate::ui::image_diff::tests::{image_line, preview};
use crate::ui::image_diff::{ImageDrawState, PreparationFrame, Rect, draw_image_line};

#[test]
#[ignore = "release-mode synthetic native image frame timings"]
fn native_image_frame_benchmark() {
    let theme = Theme::new(ThemeName::Quinjet, Appearance::Dark);
    for (width, height) in [(1920, 1080), (3840, 2160)] {
        for protocol in [
            ImageProtocol::Kitty,
            ImageProtocol::Iterm2,
            ImageProtocol::Sixel,
        ] {
            IMAGE_PREPARATION.with(|preparation| {
                *preparation.borrow_mut() = ImagePreparation::default();
            });
            let mut image = preview(ImageSide::New);
            image.width = Some(width);
            image.height = Some(height);
            image.rows = 40;
            image.cells = vec![
                ImageCell {
                    upper: [255, 0, 0],
                    lower: [0, 0, 255]
                };
                80
            ];
            image.raster = Some(Arc::new(ImageRaster {
                width,
                height,
                rgba: (0..width * height)
                    .flat_map(|pixel| {
                        let [red, green, blue, _] = pixel.to_le_bytes();
                        [red, green, blue, 255]
                    })
                    .collect(),
            }));
            let rows = (0..image.rows)
                .map(|row| {
                    let mut image = image.clone();
                    image.row = row;
                    image_line(image)
                })
                .collect::<Vec<_>>();
            let mut terminal =
                Terminal::new(TestBackend::new(100, 44)).expect("benchmark terminal");
            let render = |frame: &mut ratatui::Frame<'_>| {
                let _images = PreparationFrame::begin();
                let mut state = ImageDrawState::new(true);
                for (offset, line) in rows.iter().enumerate() {
                    let row = u16::try_from(offset).expect("benchmark row");
                    draw_image_line(
                        frame,
                        Rect::new(0, row, 100, 1),
                        line,
                        44 - row,
                        &theme,
                        protocol,
                        &mut state,
                    );
                }
            };
            let started = Instant::now();
            let _frame = terminal.draw(render).expect("cold image frame");
            let cold = started.elapsed();
            IMAGE_PREPARATION.with(|preparation| {
                let mut preparation = preparation.borrow_mut();
                let completed = preparation
                    .encoder
                    .as_ref()
                    .expect("benchmark encoder")
                    .completed
                    .recv_timeout(Duration::from_secs(60))
                    .expect("prepared benchmark image");
                assert!(
                    preparation.accept(completed),
                    "benchmark image becomes native"
                );
            });
            let ready = started.elapsed();
            let _frame = terminal.draw(render).expect("initial native frame");
            assert!(
                terminal
                    .backend()
                    .buffer()
                    .cell((0, 0))
                    .expect("native image")
                    .symbol()
                    .contains('\x1b'),
                "benchmark measures prepared native images"
            );
            let mut frames = Vec::new();
            for _ in 0..20 {
                let started = Instant::now();
                let _frame = terminal.draw(render).expect("warm image frame");
                frames.push(started.elapsed());
            }
            frames.sort_unstable();
            let warm = frames.get(10).expect("median warm frame");
            writeln!(
                std::io::stdout(),
                "{width}x{height} {protocol:?} pending_us={} native_ready_us={} warm_median_us={}",
                cold.as_micros(),
                ready.as_micros(),
                warm.as_micros()
            )
            .expect("benchmark output");
        }
    }
    IMAGE_PREPARATION.with(|preparation| {
        *preparation.borrow_mut() = ImagePreparation::default();
    });
}
