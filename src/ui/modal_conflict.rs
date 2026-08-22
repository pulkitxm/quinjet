#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_conflict(frame: &mut Frame<'_>, change: &Change, theme: &Theme) {
    let area = centered_rect(
        frame.area().width.saturating_sub(14).min(72),
        10,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Resolve Merge Conflict ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                file_icon_span(&change.path, theme),
                Span::raw(" "),
                Span::styled(
                    change.display_path(),
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "o",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" accept ours     "),
                Span::styled(
                    "t",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" accept theirs     "),
                Span::styled(
                    "s",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" mark resolved"),
            ]),
        ]),
        Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(2),
        ),
    );
    draw_modal_hint(frame, area, "Esc cancel", theme);
}
