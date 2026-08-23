#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_conflict(
    frame: &mut Frame<'_>,
    hits: &mut Vec<(Rect, ModalAction)>,
    change: &Change,
    theme: &Theme,
) {
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
    let actions_y = inner.y.saturating_add(3);
    let ours = Rect::new(inner.x, actions_y, 18.min(inner.width), 1);
    let theirs = Rect::new(
        ours.right(),
        actions_y,
        20.min(inner.right().saturating_sub(ours.right())),
        1,
    );
    let resolved = Rect::new(
        theirs.right(),
        actions_y,
        inner.right().saturating_sub(theirs.right()),
        1,
    );
    hits.push((ours, ModalAction::ConflictOurs));
    hits.push((theirs, ModalAction::ConflictTheirs));
    hits.push((resolved, ModalAction::ConflictResolved));
    draw_modal_hint(frame, area, "Esc cancel", theme);
}
