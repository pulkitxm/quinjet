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
    target: Option<ScrollTarget>,
    last: Instant,
    blocked: bool,
}

impl App {
    fn mouse_scroll_target(&self, event: MouseEvent) -> Option<ScrollTarget> {
        if let Some(modal) = &self.modal {
            return Some(ScrollTarget::Modal(std::mem::discriminant(modal)));
        }
        let point = (event.column, event.row).into();
        if matches!(
            event.kind,
            MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight
        ) || (matches!(
            event.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) && event.modifiers.contains(KeyModifiers::SHIFT))
        {
            return self
                .horizontal_scroll_regions
                .iter()
                .any(|area| area.contains(point))
                .then_some(ScrollTarget::Content);
        }
        if self.geometry.sidebar.contains(point) {
            Some(ScrollTarget::Sidebar)
        } else if self.geometry.content.contains(point) {
            Some(ScrollTarget::Content)
        } else {
            None
        }
    }

    pub(super) fn accept_mouse_scroll(&mut self, event: MouseEvent, now: Instant) -> bool {
        let target = self.mouse_scroll_target(event);
        let wheel = matches!(
            event.kind,
            MouseEventKind::ScrollUp
                | MouseEventKind::ScrollDown
                | MouseEventKind::ScrollLeft
                | MouseEventKind::ScrollRight
        );
        if wheel {
            self.mouse_hover = None;
            self.link_hover = None;
            if target.is_none() && self.mouse_scroll.is_none() {
                return false;
            }
            let burst = self.mouse_scroll.get_or_insert(ScrollBurst {
                target,
                last: now,
                blocked: false,
            });
            if now.saturating_duration_since(burst.last) >= SCROLL_IDLE {
                *burst = ScrollBurst {
                    target,
                    last: now,
                    blocked: false,
                };
            }
            burst.last = now;
            burst.blocked |= burst.target != target;
            !burst.blocked
        } else {
            if event.kind == MouseEventKind::Moved {
                if let Some(burst) = self.mouse_scroll.as_mut()
                    && now.saturating_duration_since(burst.last) < SCROLL_IDLE
                {
                    burst.blocked |= burst.target != target;
                }
            } else {
                self.mouse_scroll = None;
            }
            true
        }
    }
}
