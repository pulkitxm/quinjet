use std::fmt::Write;
use std::num::NonZeroU16;

use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::widgets::Widget;
use ratatui_image::Image;
use ratatui_image::protocol::Protocol;

use super::{Frame, Rect};

pub(super) struct KittyPlacement {
    prefix: String,
    rows: Vec<String>,
    width: u16,
    transmitted: bool,
}

impl KittyPlacement {
    pub(super) fn new(encoded: &Protocol) -> Option<Self> {
        let area = encoded.area();
        if area.width == 0 || area.height == 0 {
            return None;
        }
        let mut buffer = Buffer::empty(Rect::new(0, 0, area.width, area.height));
        Image::new(encoded).render(buffer.area, &mut buffer);
        let mut prefix = String::new();
        let mut rows = Vec::with_capacity(usize::from(area.height));
        for row in 0..area.height {
            let symbol = buffer.cell((0, row))?.symbol();
            let (before_restore, _) = symbol.split_once("\x1b[u")?;
            let (before_save, body) = before_restore.split_once("\x1b[s")?;
            if row == 0 {
                prefix.push_str(before_save);
            }
            let mut prepared = String::from("\x1b[s");
            prepared.push_str(body);
            rows.push(prepared);
        }
        Some(Self {
            prefix,
            rows,
            width: area.width,
            transmitted: false,
        })
    }

    pub(super) fn render(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        row_offset: u16,
        available_height: u16,
    ) -> bool {
        let offset = usize::from(row_offset);
        let visible = self
            .rows
            .len()
            .saturating_sub(offset)
            .min(usize::from(available_height));
        if visible == 0 {
            return true;
        }
        let Some(height) = u16::try_from(visible).ok() else {
            return false;
        };
        for row in 0..height {
            let Some(body) = self.rows.get(offset + usize::from(row)) else {
                return false;
            };
            let mut symbol = String::new();
            if !self.transmitted && row == 0 {
                symbol.push_str(&self.prefix);
                self.transmitted = true;
            }
            symbol.push_str(body);
            let _result = write!(
                symbol,
                "\x1b[u\x1b[{}C\x1b[{}B",
                self.width.saturating_sub(1),
                height.saturating_sub(1)
            );
            let y = area.y.saturating_add(row);
            let Some(cell) = frame.buffer_mut().cell_mut((area.x, y)) else {
                return false;
            };
            let _cell = cell.set_symbol(&symbol);
            cell.diff_option = CellDiffOption::ForcedWidth(NonZeroU16::MIN);
            for column in 1..self.width {
                if let Some(cell) = frame
                    .buffer_mut()
                    .cell_mut((area.x.saturating_add(column), y))
                {
                    cell.diff_option = CellDiffOption::Skip;
                }
            }
        }
        true
    }
}
