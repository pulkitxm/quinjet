#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use crate::app::graft_tools::{GraftPicker, GraftResult};

pub(super) fn draw_graft_tools(
    frame: &mut Frame<'_>,
    picker: &GraftPicker,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let notes = cells(picker.notes.len());
    let height = cells(picker.rows.len())
        .saturating_add(notes)
        .saturating_add(5)
        .max(8);
    let area = centered_rect(
        frame.area().width.saturating_sub(4).min(96),
        height.min(frame.area().height.saturating_sub(4)),
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Graftcode gateway tools ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let body = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(2),
    );
    if let Some(running) = &picker.running {
        frame.render_widget(
            Paragraph::new(format!(
                "Running {running}…\n\nEsc stops waiting; the gateway keeps working."
            ))
            .style(Style::default().fg(theme.muted)),
            body,
        );
        draw_modal_hint(frame, area, "Esc close", theme);
        return;
    }
    if !picker.ready {
        frame.render_widget(
            Paragraph::new("Asking registered gateways for their tools…")
                .style(Style::default().fg(theme.muted)),
            body,
        );
        draw_modal_hint(frame, area, "Esc cancel", theme);
        return;
    }
    let rows_height = body.height.saturating_sub(notes);
    let offset = list.offset(picker.selected, usize::from(rows_height), picker.rows.len());
    for (row, entry) in picker
        .rows
        .iter()
        .skip(offset)
        .take(usize::from(rows_height))
        .enumerate()
    {
        let index = offset.saturating_add(row);
        let active = index == picker.selected;
        let background = if active { theme.selected } else { theme.panel };
        let row_area = Rect::new(body.x, body.y.saturating_add(cells(row)), body.width, 1);
        let description = entry
            .tool
            .description
            .as_deref()
            .and_then(|text| text.lines().next())
            .unwrap_or_default();
        let filled = if entry.fills_everything() {
            ""
        } else {
            " (some arguments empty)"
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{}{}", if active { " › " } else { "   " }, entry.label()),
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {description}{filled}"),
                    Style::default().fg(theme.muted),
                ),
            ]))
            .style(Style::default().bg(background)),
            row_area,
        );
        list.hit(row_area, index);
    }
    for (row, note) in picker.notes.iter().enumerate() {
        frame.render_widget(
            Paragraph::new(note.as_str()).style(Style::default().fg(theme.muted)),
            Rect::new(
                body.x,
                body.y
                    .saturating_add(rows_height)
                    .saturating_add(cells(row)),
                body.width,
                1,
            ),
        );
    }
    draw_modal_hint(
        frame,
        area,
        "↑/↓ choose  Enter run with repository context  Esc cancel",
        theme,
    );
}

pub(super) fn draw_graft_result(frame: &mut Frame<'_>, view: &GraftResult, theme: &Theme) {
    let width = frame.area().width.saturating_sub(4).min(110);
    let wrapped: usize = view
        .text
        .lines()
        .map(|line| {
            UnicodeWidthStr::width(line)
                .div_ceil(usize::from(width.saturating_sub(2)).max(1))
                .max(1)
        })
        .sum();
    let height = cells(wrapped)
        .saturating_add(5)
        .clamp(8, frame.area().height.saturating_sub(4).clamp(1, 36));
    let area = centered_rect(width, height, frame.area());
    frame.render_widget(Clear, area);
    let block = modal_block(&format!(" {} ", view.title), theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let body = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(2),
    );
    let color = if view.is_error {
        theme.error
    } else {
        theme.text
    };
    frame.render_widget(
        Paragraph::new(view.text.as_str())
            .style(Style::default().fg(color))
            .wrap(Wrap { trim: false })
            .scroll((u16::try_from(view.scroll).unwrap_or(u16::MAX), 0)),
        body,
    );
    let hint = if view.commit_message {
        "Enter commit with this message  c copy  ↑/↓ scroll  Esc close"
    } else {
        "c copy  ↑/↓ PgUp/PgDn scroll  Esc close"
    };
    draw_modal_hint(frame, area, hint, theme);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::app::graft_tools::GraftRow;
    use crate::graft::client::{Tool, ToolParam};
    use crate::graft::registry::Gateway;

    fn screen(draw: impl FnOnce(&mut Frame<'_>, &mut ModalList<'_>)) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        let mut hits = Vec::new();
        let mut len = 0;
        let mut max_scroll = 0;
        let rendered = terminal
            .draw(|frame| {
                let mut list = ModalList::new(&mut hits, &mut len, &mut max_scroll, 0, false);
                draw(frame, &mut list);
            })
            .unwrap();
        rendered
            .buffer
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    fn row(name: &str, param: &str) -> GraftRow {
        GraftRow {
            gateway: Gateway {
                name: "agent".to_owned(),
                url: "http://localhost:7390/mcp".to_owned(),
                token_env: None,
            },
            tool: Tool {
                name: name.to_owned(),
                title: None,
                description: Some("Draft a commit message\nsecond line".to_owned()),
                params: vec![ToolParam {
                    name: param.to_owned(),
                    description: None,
                    required: true,
                }],
            },
        }
    }

    #[test]
    fn the_picker_lists_tools_and_flags_arguments_it_cannot_fill() {
        let picker = GraftPicker {
            rows: vec![
                row("draft_commit_message", "staged_diff"),
                row("echo", "text"),
            ],
            selected: 0,
            ready: true,
            notes: vec!["down is unavailable: refused".to_owned()],
            running: None,
        };
        let text = screen(|frame, list| draw_graft_tools(frame, &picker, list, &Theme::default()));
        assert!(text.contains(" › agent › draft_commit_message  Draft a commit message"));
        assert!(!text.contains("second line"));
        assert!(text.contains("agent › echo  Draft a commit message (some arguments empty)"));
        assert!(text.contains("down is unavailable: refused"));
        assert!(text.contains("Enter run with repository context"));
    }

    #[test]
    fn running_and_finished_calls_have_their_own_screens() {
        let running = GraftPicker {
            running: Some("agent › investigate_pull_request".to_owned()),
            ready: true,
            ..GraftPicker::default()
        };
        let text = screen(|frame, list| draw_graft_tools(frame, &running, list, &Theme::default()));
        assert!(text.contains("Running agent › investigate_pull_request…"));
        let result = GraftResult {
            title: "agent › draft_commit_message".to_owned(),
            text: "fix: keep the cursor in view".to_owned(),
            is_error: false,
            scroll: 0,
            commit_message: true,
        };
        let text = screen(|frame, _| draw_graft_result(frame, &result, &Theme::default()));
        assert!(text.contains(" agent › draft_commit_message "));
        assert!(text.contains("fix: keep the cursor in view"));
        assert!(text.contains("Enter commit with this message"));
    }
}
