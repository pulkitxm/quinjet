#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

const SCROLL_IDLE: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollTarget {
    Sidebar,
    Content,
    Modal(std::mem::Discriminant<Modal>),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScrollBurst {
    target: ScrollTarget,
    position: (u16, u16),
    last: Instant,
}

impl App {
    fn mouse_scroll_target(&self, event: MouseEvent) -> Option<ScrollTarget> {
        if let Some(modal) = &self.modal {
            return Some(ScrollTarget::Modal(std::mem::discriminant(modal)));
        }
        let point = (event.column, event.row).into();
        if self.geometry.sidebar.contains(point) {
            Some(ScrollTarget::Sidebar)
        } else if self.geometry.content.contains(point)
            || self
                .horizontal_scroll_regions
                .iter()
                .any(|area| area.contains(point))
        {
            Some(ScrollTarget::Content)
        } else {
            None
        }
    }

    pub(super) fn route_mouse_scroll(
        &mut self,
        mut event: MouseEvent,
        now: Instant,
    ) -> Option<MouseEvent> {
        let wheel = matches!(
            event.kind,
            MouseEventKind::ScrollUp
                | MouseEventKind::ScrollDown
                | MouseEventKind::ScrollLeft
                | MouseEventKind::ScrollRight
        );
        if !wheel {
            if event.kind != MouseEventKind::Moved {
                self.mouse_scroll = None;
            }
            return Some(event);
        }
        self.mouse_hover = None;
        self.link_hover = None;
        let target = self.mouse_scroll_target(event);
        let horizontal = matches!(
            event.kind,
            MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight
        ) || event.modifiers.contains(KeyModifiers::SHIFT);
        let mut burst = match self.mouse_scroll {
            Some(burst) if now.saturating_duration_since(burst.last) < SCROLL_IDLE => burst,
            _ => {
                if horizontal
                    && self.modal.is_none()
                    && !self
                        .horizontal_scroll_regions
                        .iter()
                        .any(|area| area.contains((event.column, event.row).into()))
                {
                    return None;
                }
                ScrollBurst {
                    target: target?,
                    position: (event.column, event.row),
                    last: now,
                }
            }
        };
        burst.last = now;
        self.mouse_scroll = Some(burst);
        if Some(burst.target) == target {
            return Some(event);
        }
        let area = match burst.target {
            ScrollTarget::Sidebar if self.modal.is_none() => self.geometry.sidebar,
            ScrollTarget::Content if self.modal.is_none() => self.geometry.content,
            _ => return None,
        };
        if area.width == 0 || area.height == 0 {
            return None;
        }
        event.column = burst.position.0.clamp(area.x, area.right() - 1);
        event.row = burst.position.1.clamp(area.y, area.bottom() - 1);
        Some(event)
    }
}
