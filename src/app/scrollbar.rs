#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Scrollbar {
    pub track: Rect,
    pub thumb: Rect,
    pub max_offset: usize,
    pub viewport: usize,
}

impl Scrollbar {
    pub(crate) fn new(area: Rect, offset: usize, length: usize, viewport: usize) -> Option<Self> {
        if area.width == 0 || area.height < 2 || viewport == 0 || length <= viewport {
            return None;
        }
        let height = usize::from(area.height);
        let thumb_height = height
            .saturating_mul(viewport)
            .div_euclid(length)
            .clamp(1, height - 1);
        let max_offset = length - viewport;
        let start = proportional(offset.min(max_offset), height - thumb_height, max_offset);
        let track = Rect::new(area.right().saturating_sub(1), area.y, 1, area.height);
        let thumb = Rect::new(
            track.x,
            track.y.saturating_add(crate::convert::cells(start)),
            1,
            crate::convert::cells(thumb_height),
        );
        Some(Self {
            track,
            thumb,
            max_offset,
            viewport,
        })
    }

    fn offset_at(self, row: u16, grab_offset: u16) -> usize {
        let travel = usize::from(self.track.height.saturating_sub(self.thumb.height));
        let start = usize::from(
            row.saturating_sub(self.track.y)
                .saturating_sub(grab_offset.min(self.thumb.height.saturating_sub(1))),
        )
        .min(travel);
        proportional(start, self.max_offset, travel)
    }
}

fn proportional(value: usize, scale: usize, total: usize) -> usize {
    usize::try_from(
        (value as u128 * scale as u128 + (total as u128).div_euclid(2))
            .checked_div(total as u128)
            .unwrap_or_default(),
    )
    .unwrap_or(usize::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScrollbarDrag {
    focus: Focus,
    grab_offset: u16,
    row: u16,
}

impl App {
    pub(super) fn handle_scrollbar_mouse(
        &mut self,
        event: MouseEvent,
        effects: &mut Vec<AppEffect>,
    ) -> bool {
        if self.modal.is_some()
            || self.repository_tab_menu.is_some()
            || self.scm_menu_open
            || self.pr_menu_open
            || !self.mouse_capture
        {
            self.scrollbar_drag = None;
            return false;
        }
        if let Some(mut drag) = self.scrollbar_drag {
            let scrollbar = match drag.focus {
                Focus::Sidebar => self.geometry.sidebar_scrollbar,
                Focus::Content => self.geometry.content_scrollbar,
            };
            let Some(scrollbar) = scrollbar else {
                self.scrollbar_drag = None;
                return false;
            };
            match event.kind {
                MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                    if event.row != drag.row {
                        self.apply_scrollbar_offset(
                            drag.focus,
                            scrollbar.offset_at(event.row, drag.grab_offset),
                            scrollbar,
                            effects,
                        );
                        drag.row = event.row;
                    }
                    self.scrollbar_drag =
                        (event.kind != MouseEventKind::Up(MouseButton::Left)).then_some(drag);
                    return true;
                }
                MouseEventKind::Down(_) => self.scrollbar_drag = None,
                _ => return true,
            }
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return false;
        }
        let point = (event.column, event.row).into();
        let hit = [
            (Focus::Sidebar, self.geometry.sidebar_scrollbar),
            (Focus::Content, self.geometry.content_scrollbar),
        ]
        .into_iter()
        .find_map(|(focus, scrollbar)| {
            scrollbar
                .filter(|scrollbar| scrollbar.track.contains(point))
                .map(|scrollbar| (focus, scrollbar))
        });
        let Some((focus, scrollbar)) = hit else {
            return false;
        };
        self.text_selection = None;
        self.resize_target = None;
        self.last_resize_tap = None;
        self.set_focus(focus, effects);
        let in_thumb = scrollbar.thumb.contains(point);
        let grab_offset = if in_thumb {
            event.row.saturating_sub(scrollbar.thumb.y)
        } else {
            scrollbar.thumb.height.div_euclid(2)
        };
        if !in_thumb {
            self.apply_scrollbar_offset(
                focus,
                scrollbar.offset_at(event.row, grab_offset),
                scrollbar,
                effects,
            );
        }
        self.scrollbar_drag = Some(ScrollbarDrag {
            focus,
            grab_offset,
            row: event.row,
        });
        true
    }

    fn apply_scrollbar_offset(
        &mut self,
        focus: Focus,
        offset: usize,
        scrollbar: Scrollbar,
        effects: &mut Vec<AppEffect>,
    ) {
        match focus {
            Focus::Sidebar => {
                self.sidebar_free_scroll = true;
                self.sidebar_offset = offset;
                self.request_history_near_end(offset.saturating_add(scrollbar.viewport), effects);
            }
            Focus::Content => {
                self.content_file_anchor = None;
                self.content_scroll = offset;
                self.content_at_bottom = offset == scrollbar.max_offset;
            }
        }
    }
}
