#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::integer_division,
        reason = "scroll positions map to whole terminal cells"
    )]
    pub(super) fn handle_horizontal_scrollbar_mouse(&mut self, event: MouseEvent) -> bool {
        if event.kind == MouseEventKind::Up(MouseButton::Left) {
            return self.horizontal_scroll_drag.take().is_some();
        }
        let Some(bar) = self.geometry.horizontal_scrollbar else {
            self.horizontal_scroll_drag = None;
            return false;
        };
        let track_start = bar.area.x.saturating_add(1);
        let travel = bar
            .area
            .width
            .saturating_sub(2)
            .saturating_sub(bar.thumb.width)
            .max(1);
        if event.kind == MouseEventKind::Drag(MouseButton::Left) {
            let Some(drag) = self.horizontal_scroll_drag else {
                return false;
            };
            let amount = usize::from(event.column.abs_diff(drag.column))
                .saturating_mul(self.horizontal_scroll_limit)
                / usize::from(travel);
            self.horizontal_scroll = if event.column < drag.column {
                drag.offset.saturating_sub(amount)
            } else {
                drag.offset
                    .saturating_add(amount)
                    .min(self.horizontal_scroll_limit)
            };
            return true;
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return false;
        }
        self.horizontal_scroll_drag = None;
        if !bar.area.contains((event.column, event.row).into()) {
            return false;
        }
        self.text_selection = None;
        self.focus = Focus::Content;
        self.resize_target = None;
        if event.column == bar.area.x {
            self.scroll_horizontal(false);
        } else if event.column == bar.area.right().saturating_sub(1) {
            self.scroll_horizontal(true);
        } else {
            if !bar.thumb.contains((event.column, event.row).into()) {
                let position = event
                    .column
                    .saturating_sub(track_start)
                    .saturating_sub(bar.thumb.width / 2)
                    .min(travel);
                self.horizontal_scroll = usize::from(position)
                    .saturating_mul(self.horizontal_scroll_limit)
                    / usize::from(travel);
            }
            self.horizontal_scroll_drag = Some(HorizontalScrollDrag {
                column: event.column,
                offset: self.horizontal_scroll,
            });
        }
        true
    }
}
