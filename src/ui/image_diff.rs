use std::cell::RefCell;
use std::num::NonZeroU16;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use image::{DynamicImage, RgbaImage};
use ratatui::buffer::CellDiffOption;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize};
use unicode_width::UnicodeWidthStr;

use super::{DiffLine, Frame, Rect, Theme};
use crate::git::diff::{ImagePreview, ImageProtocol, ImageRaster, ImageSide};

mod kitty;
use kitty::KittyPlacement;

static IMAGE_PICKER: OnceLock<(Picker, ImageProtocol)> = OnceLock::new();
thread_local! {
    static ENCODED_IMAGES: RefCell<Vec<EncodedImage>> = const { RefCell::new(Vec::new()) };
}

struct EncodedImage {
    raster: Arc<ImageRaster>,
    protocol: ImageProtocol,
    width: u16,
    height: u16,
    display: NativeDisplay,
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

pub(crate) fn initialize_image_picker() {
    let _state = IMAGE_PICKER.get_or_init(|| {
        let inferred = ImageProtocol::detect();
        let override_value = std::env::var("QUINJET_IMAGE_PROTOCOL")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let query = should_query(inferred, &override_value);
        let picker = if query {
            Picker::from_query_stdio_with_options(QueryStdioOptions {
                timeout: Duration::from_millis(500),
                text_sizing_protocol: false,
            })
            .unwrap_or_else(|_| Picker::halfblocks())
        } else {
            Picker::halfblocks()
        };
        let protocol = choose_protocol(inferred, &override_value, picker.protocol_type());
        (picker, protocol)
    });
}

fn should_query(inferred: ImageProtocol, override_value: &str) -> bool {
    inferred.is_native() || override_value == "auto"
}

fn choose_protocol(
    inferred: ImageProtocol,
    override_value: &str,
    queried: ProtocolType,
) -> ImageProtocol {
    if override_value == "halfblocks" || inferred.is_native() {
        return inferred;
    }
    match queried {
        ProtocolType::Kitty => ImageProtocol::Kitty,
        ProtocolType::Iterm2 => ImageProtocol::Iterm2,
        ProtocolType::Sixel => ImageProtocol::Sixel,
        ProtocolType::Halfblocks => ImageProtocol::Halfblocks,
    }
}

pub(super) fn selected_image_protocol() -> ImageProtocol {
    IMAGE_PICKER
        .get()
        .map_or_else(ImageProtocol::detect, |(_, protocol)| *protocol)
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
    if protocol.is_native() {
        if let Some(raster) = preview.raster.as_ref() {
            if state.contains(raster) {
                return;
            }
            if (protocol == ImageProtocol::Kitty
                || (state.allow_other_native
                    && preview.row == 0
                    && remaining_height >= preview.rows))
                && draw_native(frame, area, preview, protocol, remaining_height)
            {
                state.remember(raster);
                return;
            }
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
    let rendered = ENCODED_IMAGES.with(|cache| {
        let mut cache = cache.borrow_mut();
        let position = cache.iter().position(|entry| {
            Arc::ptr_eq(&entry.raster, raster)
                && entry.protocol == protocol
                && entry.width == width
                && entry.height == height
        });
        if let Some(position) = position {
            let Some(entry) = cache.get_mut(position) else {
                return false;
            };
            return render_native(frame, area, preview.row, remaining_height, entry);
        }
        let Some(image) = raster_dynamic(raster) else {
            return false;
        };
        let mut picker = IMAGE_PICKER
            .get()
            .map_or_else(Picker::halfblocks, |(picker, _)| picker.clone());
        picker.set_protocol_type(match protocol {
            ImageProtocol::Kitty => ProtocolType::Kitty,
            ImageProtocol::Iterm2 => ProtocolType::Iterm2,
            ImageProtocol::Sixel => ProtocolType::Sixel,
            ImageProtocol::Halfblocks => ProtocolType::Halfblocks,
        });
        let Ok(encoded) =
            picker.new_protocol(image, Rect::new(0, 0, width, height), Resize::Fit(None))
        else {
            return false;
        };
        if cache.len() == 4 {
            drop(cache.remove(0));
        }
        let display = if protocol == ImageProtocol::Kitty {
            let Some(placement) = KittyPlacement::new(&encoded) else {
                return false;
            };
            NativeDisplay::Kitty(placement)
        } else {
            NativeDisplay::Other(encoded)
        };
        cache.push(EncodedImage {
            raster: Arc::clone(raster),
            protocol,
            width,
            height,
            display,
        });
        let Some(entry) = cache.last_mut() else {
            return false;
        };
        render_native(frame, area, preview.row, remaining_height, entry)
    });
    if !rendered {
        return false;
    }
    true
}

fn render_native(
    frame: &mut Frame<'_>,
    area: Rect,
    row_offset: u16,
    remaining_height: u16,
    entry: &mut EncodedImage,
) -> bool {
    match &mut entry.display {
        NativeDisplay::Kitty(placement) => {
            placement.render(frame, area, row_offset, remaining_height)
        }
        NativeDisplay::Other(encoded) => {
            frame.render_widget(
                Image::new(encoded),
                Rect::new(area.x, area.y, entry.width, entry.height),
            );
            for row in 0..entry.height {
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
