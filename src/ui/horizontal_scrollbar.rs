#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw(
    frame: &mut Frame<'_>,
    content: Rect,
    app: &App,
    theme: &Theme,
) -> Option<crate::app::Scrollbar> {
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
    let scrollbar = crate::app::Scrollbar::horizontal(
        area,
        app.horizontal_scroll,
        app.horizontal_scroll_limit,
        viewport,
    );
    draw_scrollbar(frame, scrollbar, theme);
    scrollbar
}
