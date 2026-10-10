#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Scrollbar {
    pub area: Rect,
    pub track: Rect,
    pub thumb: Rect,
    pub max_offset: usize,
    pub viewport: usize,
    pub horizontal: bool,
}

impl Scrollbar {
    pub(crate) fn new(area: Rect, offset: usize, length: usize, viewport: usize) -> Option<Self> {
        if area.width == 0 {
            return None;
        }
        let track = Rect::new(area.right().saturating_sub(1), area.y, 1, area.height);
        Self::build(track, track, offset, length, viewport, false)
    }

    pub(crate) fn horizontal(
        area: Rect,
        offset: usize,
        max_offset: usize,
        viewport: usize,
    ) -> Option<Self> {
        let track = Rect::new(
            area.x.saturating_add(1),
            area.y,
            area.width.saturating_sub(2),
            1,
        );
        Self::build(
            area,
            track,
            offset,
            max_offset.saturating_add(viewport),
            viewport,
            true,
        )
    }

    fn build(
        area: Rect,
        track: Rect,
        offset: usize,
        length: usize,
        viewport: usize,
        horizontal: bool,
    ) -> Option<Self> {
        let extent = if horizontal {
            track.width
        } else {
            track.height
        };
        if track.width == 0
            || track.height == 0
            || extent < 2
            || viewport == 0
            || length <= viewport
        {
            return None;
        }
        let extent = usize::from(extent);
        let thumb_extent = extent
            .saturating_mul(viewport)
            .div_euclid(length)
            .clamp(1, extent - 1);
        let max_offset = length - viewport;
        let start = crate::convert::cells(proportional(
            offset.min(max_offset),
            extent - thumb_extent,
            max_offset,
        ));
        let thumb_extent = crate::convert::cells(thumb_extent);
        let thumb = if horizontal {
            Rect::new(track.x.saturating_add(start), track.y, thumb_extent, 1)
        } else {
            Rect::new(track.x, track.y.saturating_add(start), 1, thumb_extent)
        };
        Some(Self {
            area,
            track,
            thumb,
            max_offset,
            viewport,
            horizontal,
        })
    }

    const fn coordinate(self, column: u16, row: u16) -> u16 {
        if self.horizontal { column } else { row }
    }

    const fn thumb_extent(self) -> u16 {
        if self.horizontal {
            self.thumb.width
        } else {
            self.thumb.height
        }
    }

    fn offset_at(self, position: u16, grab_offset: u16) -> usize {
        let extent = if self.horizontal {
            self.track.width
        } else {
            self.track.height
        };
        let travel = usize::from(extent.saturating_sub(self.thumb_extent()));
        let origin = self.coordinate(self.track.x, self.track.y);
        let start = usize::from(
            position
                .saturating_sub(origin)
                .saturating_sub(grab_offset.min(self.thumb_extent().saturating_sub(1))),
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
pub(crate) enum ScrollbarTarget {
    Sidebar,
    Content,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScrollbarDrag {
    target: ScrollbarTarget,
    grab_offset: u16,
    position: u16,
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
            let scrollbar = self.scrollbar(drag.target);
            let Some(scrollbar) = scrollbar else {
                self.scrollbar_drag = None;
                return false;
            };
            match event.kind {
                MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                    let position = scrollbar.coordinate(event.column, event.row);
                    if position != drag.position {
                        self.apply_scrollbar_offset(
                            drag.target,
                            scrollbar.offset_at(position, drag.grab_offset),
                            scrollbar,
                            effects,
                        );
                        drag.position = position;
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
            (ScrollbarTarget::Sidebar, self.geometry.sidebar_scrollbar),
            (ScrollbarTarget::Content, self.geometry.content_scrollbar),
            (
                ScrollbarTarget::Horizontal,
                self.geometry.horizontal_scrollbar,
            ),
        ]
        .into_iter()
        .find_map(|(target, scrollbar)| {
            scrollbar
                .filter(|scrollbar| scrollbar.area.contains(point))
                .map(|scrollbar| (target, scrollbar))
        });
        let Some((target, scrollbar)) = hit else {
            return false;
        };
        self.text_selection = None;
        self.resize_target = None;
        self.last_resize_tap = None;
        self.set_focus(
            if target == ScrollbarTarget::Sidebar {
                Focus::Sidebar
            } else {
                Focus::Content
            },
            effects,
        );
        if !scrollbar.track.contains(point) {
            self.scroll_horizontal(event.column > scrollbar.track.x);
            return true;
        }
        let position = scrollbar.coordinate(event.column, event.row);
        let in_thumb = scrollbar.thumb.contains(point);
        let grab_offset = if in_thumb {
            position.saturating_sub(scrollbar.coordinate(scrollbar.thumb.x, scrollbar.thumb.y))
        } else {
            scrollbar.thumb_extent().div_euclid(2)
        };
        if !in_thumb {
            self.apply_scrollbar_offset(
                target,
                scrollbar.offset_at(position, grab_offset),
                scrollbar,
                effects,
            );
        }
        self.scrollbar_drag = Some(ScrollbarDrag {
            target,
            grab_offset,
            position,
        });
        true
    }

    const fn scrollbar(&self, target: ScrollbarTarget) -> Option<Scrollbar> {
        match target {
            ScrollbarTarget::Sidebar => self.geometry.sidebar_scrollbar,
            ScrollbarTarget::Content => self.geometry.content_scrollbar,
            ScrollbarTarget::Horizontal => self.geometry.horizontal_scrollbar,
        }
    }

    pub(crate) fn cancel_unavailable_scrollbar_drag(&mut self) {
        if self.modal.is_some()
            || self.repository_tab_menu.is_some()
            || self.scm_menu_open
            || self.pr_menu_open
            || !self.mouse_capture
            || self
                .scrollbar_drag
                .is_some_and(|drag| self.scrollbar(drag.target).is_none())
        {
            self.scrollbar_drag = None;
        }
    }

    fn apply_scrollbar_offset(
        &mut self,
        target: ScrollbarTarget,
        offset: usize,
        scrollbar: Scrollbar,
        effects: &mut Vec<AppEffect>,
    ) {
        match target {
            ScrollbarTarget::Sidebar => {
                self.sidebar_free_scroll = true;
                self.sidebar_offset = offset;
                self.request_history_near_end(offset.saturating_add(scrollbar.viewport), effects);
            }
            ScrollbarTarget::Content => {
                self.content_file_anchor = None;
                self.pull_request_restore_scroll = false;
                self.diff_restore_scroll = false;
                self.content_scroll = offset;
                self.content_at_bottom = offset == scrollbar.max_offset;
            }
            ScrollbarTarget::Horizontal => self.horizontal_scroll = offset,
        }
    }
}
