#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn review_line_selected(app: &App, line: &DiffLine, old_side: Option<bool>) -> bool {
    let Some(cursor) = app.pull_request_review_cursor.as_ref() else {
        return false;
    };
    if app.pull_request_single_file.as_ref() != Some(&cursor.path) {
        return false;
    }
    match (cursor.side, old_side) {
        (crate::git::github::PullRequestReviewSide::Left, Some(false))
        | (crate::git::github::PullRequestReviewSide::Right, Some(true)) => false,
        (crate::git::github::PullRequestReviewSide::Left, _) => line.old_line == Some(cursor.line),
        (
            crate::git::github::PullRequestReviewSide::Right
            | crate::git::github::PullRequestReviewSide::Unknown,
            _,
        ) => line.new_line == Some(cursor.line),
    }
}

pub(super) fn draw_review_editor(
    frame: &mut Frame<'_>,
    title: &str,
    input: &crate::app::TextBuffer,
    decision: Option<crate::git::github::PullRequestReviewDecision>,
    theme: &Theme,
) {
    let width = frame.area().width.saturating_sub(12).min(76);
    let area = centered_rect(width, 12, frame.area());
    frame.render_widget(Clear, area);
    let block = modal_block(title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let input_y = inner.y + u16::from(decision.is_some());
    if let Some(selected) = decision {
        let labels = crate::git::github::PullRequestReviewDecision::ALL
            .iter()
            .map(|choice| {
                if *choice == selected {
                    Span::styled(
                        format!(" {} ", choice.label()),
                        Style::default()
                            .fg(theme.text)
                            .bg(theme.selected)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(
                        format!(" {} ", choice.label()),
                        Style::default().fg(theme.muted).bg(theme.panel_alt),
                    )
                }
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(Line::from(labels)).style(Style::default().bg(theme.panel_alt)),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
    }
    let input_area = Rect::new(
        inner.x,
        input_y,
        inner.width,
        inner.bottom().saturating_sub(input_y).saturating_sub(1),
    );
    frame.render_widget(
        Paragraph::new(input.value.as_str())
            .style(Style::default().fg(theme.text).bg(theme.panel_alt))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_focus)),
            ),
        input_area,
    );
    set_text_cursor(frame, input_area.inner(Margin::new(1, 1)), input, true);
    draw_modal_hint(
        frame,
        area,
        if decision.is_some() {
            "Tab decision   Ctrl+Enter submit   Esc close"
        } else {
            "Ctrl+Enter add to pending review   Esc close"
        },
        theme,
    );
}
