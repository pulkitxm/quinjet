use std::num::NonZeroU16;
use std::sync::{Arc, OnceLock};

use image::{DynamicImage, Rgba, RgbaImage, imageops};
use ratatui::buffer::CellDiffOption;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui_image::protocol::halfblocks::Halfblocks;
use ratatui_image::protocol::iterm2::Iterm2;
use ratatui_image::protocol::kitty::Kitty;
use ratatui_image::protocol::sixel::Sixel;
use ratatui_image::protocol::{ImageSource, Protocol};
use ratatui_image::{Image, Resize};
use unicode_width::UnicodeWidthStr;

use super::{App, DiffLine, Frame, Rect, Theme};
use crate::git::diff::{ImagePreview, ImageProtocol, ImageRaster, ImageSide};

mod kitty;
use kitty::KittyPlacement;
pub(super) mod picker;
use picker::ImagePicker;
pub(super) use picker::selected_image_protocol;
pub(super) mod preparation;
pub(super) use preparation::PreparationFrame;
use preparation::{EncodedImage, IMAGE_PREPARATION, ImageKey};

static IMAGE_PICKER: OnceLock<(ImagePicker, ImageProtocol)> = OnceLock::new();

pub(crate) fn draw(frame: &mut Frame<'_>, app: &mut App, theme: &Theme) {
    let _images_ready = crate::ui::image_preparation_ready();
    let _image_frame = PreparationFrame::begin();
    super::layout::draw(frame, app, theme);
}

enum NativeDisplay {
    Kitty(KittyPlacement),
    Other(Protocol),
}

#[derive(Default)]
pub(super) struct ImageDrawState {
    active: Vec<Arc<ImageRaster>>,
    allow_other_native: bool,
}

impl ImageDrawState {
    pub(super) const fn new(allow_other_native: bool) -> Self {
        Self {
            active: Vec::new(),
            allow_other_native,
        }
    }

    fn contains(&self, raster: &Arc<ImageRaster>) -> bool {
        self.active.iter().any(|active| Arc::ptr_eq(active, raster))
    }

    fn remember(&mut self, raster: &Arc<ImageRaster>) {
        self.active.push(Arc::clone(raster));
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "image rows need both viewport and frame state"
)]
pub(super) fn draw_image_line(
    frame: &mut Frame<'_>,
    area: Rect,
    line: &DiffLine,
    remaining_height: u16,
    theme: &Theme,
    protocol: ImageProtocol,
    state: &mut ImageDrawState,
) {
    if area.width == 0 {
        return;
    }
    let Some(preview) = line.image.as_ref() else {
        draw_caption(frame, area, &line.text(), theme);
        return;
    };
    if protocol.is_native()
        && let Some(raster) = preview.raster.as_ref()
    {
        if state.contains(raster) {
            return;
        }
        if (protocol == ImageProtocol::Kitty
            || (state.allow_other_native && preview.row == 0 && remaining_height >= preview.rows))
            && draw_native(frame, area, preview, protocol, remaining_height)
        {
            state.remember(raster);
            return;
        }
    }
    if preview.cells.is_empty() {
        draw_caption(frame, area, &preview.caption(), theme);
        return;
    }
    draw_halfblocks(frame, area, preview, theme);
}

fn draw_caption(frame: &mut Frame<'_>, area: Rect, text: &str, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            text.to_owned(),
            Style::default().fg(theme.muted),
        )))
        .style(Style::default().bg(theme.panel)),
        area,
    );
}

fn draw_halfblocks(frame: &mut Frame<'_>, area: Rect, preview: &ImagePreview, theme: &Theme) {
    let mut spans = Vec::with_capacity(preview.cells.len().saturating_add(1));
    if preview.row == 0 {
        spans.push(Span::styled(
            format!("{} ", preview.caption()),
            Style::default().fg(theme.muted),
        ));
    }
    let used = spans
        .first()
        .map_or(0, |span| UnicodeWidthStr::width(span.content.as_ref()));
    let remaining = (area.width as usize).saturating_sub(used);
    for cell in preview.cells.iter().take(remaining) {
        spans.push(Span::styled(
            "▀",
            Style::default()
                .fg(Color::Rgb(cell.upper[0], cell.upper[1], cell.upper[2]))
                .bg(Color::Rgb(cell.lower[0], cell.lower[1], cell.lower[2])),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.panel)),
        area,
    );
}

fn draw_native(
    frame: &mut Frame<'_>,
    area: Rect,
    preview: &ImagePreview,
    protocol: ImageProtocol,
    remaining_height: u16,
) -> bool {
    let Some(raster) = preview.raster.as_ref() else {
        return false;
    };
    let width = u16::try_from(preview.cells.len())
        .unwrap_or(area.width)
        .min(area.width);
    let height = preview.rows.max(1);
    if width == 0 {
        return false;
    }
    IMAGE_PREPARATION.with(|preparation| {
        let mut preparation = preparation.borrow_mut();
        let key = ImageKey {
            raster: Arc::clone(raster),
            protocol,
            width,
            height,
        };
        let Some(entry) = preparation.display(key) else {
            return false;
        };
        render_native(frame, area, preview.row, remaining_height, entry)
    })
}

fn prepare_native(key: &ImageKey) -> Option<NativeDisplay> {
    let image = raster_dynamic(&key.raster)?;
    let picker = IMAGE_PICKER
        .get()
        .map_or_else(picker::fallback_picker, |(picker, _)| *picker);
    let encoded = encode_native(
        image,
        key.protocol,
        Rect::new(0, 0, key.width, key.height),
        picker,
    )?;
    if key.protocol == ImageProtocol::Kitty {
        KittyPlacement::new(&encoded).map(NativeDisplay::Kitty)
    } else {
        Some(NativeDisplay::Other(encoded))
    }
}

fn encode_native(
    image: DynamicImage,
    protocol: ImageProtocol,
    size: Rect,
    picker: ImagePicker,
) -> Option<Protocol> {
    let source = ImageSource::new(image, picker.font_size, Rgba([0, 0, 0, 0]));
    let (image, area) = match Resize::Fit(None).needs_resize(
        &source,
        picker.font_size,
        source.desired,
        size,
        false,
    ) {
        Some(area) => {
            let width = u32::from(area.width) * u32::from(picker.font_size.0);
            let height = u32::from(area.height) * u32::from(picker.font_size.1);
            let resized = source
                .image
                .resize(width, height, imageops::FilterType::Nearest);
            let mut padded = DynamicImage::new_rgba8(width, height);
            imageops::overlay(&mut padded, &resized, 0, 0);
            (padded, area)
        }
        None => (source.image, source.desired),
    };
    match protocol {
        ImageProtocol::Halfblocks => Halfblocks::new(image, area).ok().map(Protocol::Halfblocks),
        ImageProtocol::Sixel => Sixel::new(image, area, picker.is_tmux)
            .ok()
            .map(Protocol::Sixel),
        ImageProtocol::Kitty => Kitty::new(image, area, rand::random(), picker.is_tmux)
            .ok()
            .map(Protocol::Kitty),
        ImageProtocol::Iterm2 => Iterm2::new(image, area, picker.is_tmux)
            .ok()
            .map(Protocol::ITerm2),
    }
}

fn render_native(
    frame: &mut Frame<'_>,
    area: Rect,
    row_offset: u16,
    remaining_height: u16,
    entry: &mut EncodedImage,
) -> bool {
    let Some(display) = entry.display.as_mut() else {
        return false;
    };
    match display {
        NativeDisplay::Kitty(placement) => {
            placement.render(frame, area, row_offset, remaining_height)
        }
        NativeDisplay::Other(encoded) => {
            frame.render_widget(
                Image::new(encoded),
                Rect::new(area.x, area.y, entry.key.width, entry.key.height),
            );
            for row in 0..entry.key.height {
                if let Some(cell) = frame
                    .buffer_mut()
                    .cell_mut((area.x, area.y.saturating_add(row)))
                {
                    cell.diff_option = CellDiffOption::ForcedWidth(NonZeroU16::MIN);
                }
            }
            true
        }
    }
}

fn raster_dynamic(raster: &ImageRaster) -> Option<DynamicImage> {
    RgbaImage::from_raw(raster.width, raster.height, raster.rgba.clone())
        .map(DynamicImage::ImageRgba8)
}

pub(super) fn image_side(line: &DiffLine) -> Option<ImageSide> {
    line.image.as_ref().map(|preview| preview.side)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod encoding_tests;
