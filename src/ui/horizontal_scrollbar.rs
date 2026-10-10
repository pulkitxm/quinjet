use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw(
    frame: &mut Frame<'_>,
    content: Rect,
    app: &App,
    theme: &Theme,
) -> Option<HorizontalScrollbar> {
    if app.modal.is_some()
        || app.horizontal_scroll_limit == 0
        || content.width < 20
        || content.height < 3
    {
        return None;
    }
    let right_margin = if app.content_at_bottom {
        2
    } else {
        cells(JUMP_BOTTOM_LABEL.width()).saturating_add(JUMP_CONTROL_MARGIN)
    };
    let area = Rect::new(
        content.x.saturating_add(2),
        content.bottom().saturating_sub(1),
        content.width.saturating_sub(right_margin).saturating_sub(2),
        1,
    );
    let viewport = app
        .horizontal_scroll_regions
        .iter()
        .map(|area| usize::from(area.width))
        .max()?;
    let mut state = ScrollbarState::new(app.horizontal_scroll_limit.saturating_add(1))
        .position(app.horizontal_scroll)
        .viewport_content_length(viewport);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .begin_symbol(Some("◀"))
            .end_symbol(Some("▶"))
            .track_symbol(Some("─"))
            .thumb_symbol("━")
            .track_style(Style::default().fg(theme.border).bg(theme.panel))
            .thumb_style(Style::default().fg(theme.accent).bg(theme.panel))
            .begin_style(Style::default().fg(theme.accent_soft).bg(theme.panel))
            .end_style(Style::default().fg(theme.accent_soft).bg(theme.panel)),
        area,
        &mut state,
    );
    let buffer = frame.buffer_mut();
    let mut columns = (area.x..area.right()).filter(|column| {
        buffer
            .cell((*column, area.y))
            .is_some_and(|cell| cell.symbol() == "━")
    });
    let start = columns.next()?;
    let end = columns.next_back().unwrap_or(start);
    Some(HorizontalScrollbar {
        area,
        thumb: Rect::new(
            start,
            area.y,
            end.saturating_sub(start).saturating_add(1),
            1,
        ),
    })
}
