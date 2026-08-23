#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) fn help_rows(exit_locked: bool) -> Vec<&'static HelpRow> {
    HELP_ROWS
        .iter()
        .filter(|row| {
            !exit_locked
                || !matches!(
                    row,
                    HelpRow::Shortcut {
                        keys: "Ctrl+W" | "Right-click project tab" | "q",
                        ..
                    }
                )
        })
        .collect()
}

pub(crate) fn help_shortcut_count(exit_locked: bool) -> usize {
    help_rows(exit_locked)
        .iter()
        .filter(|row| matches!(row, HelpRow::Shortcut { .. }))
        .count()
}

pub(crate) fn help_display_index(selected: usize, exit_locked: bool) -> usize {
    help_rows(exit_locked)
        .iter()
        .enumerate()
        .filter_map(|(index, row)| matches!(row, HelpRow::Shortcut { .. }).then_some(index))
        .nth(selected)
        .unwrap_or(0)
}

pub(crate) fn help_shortcut_index_at(display: usize, exit_locked: bool) -> Option<usize> {
    let rows = help_rows(exit_locked);
    if !matches!(rows.get(display), Some(HelpRow::Shortcut { .. })) {
        return None;
    }
    Some(
        rows.iter()
            .take(display)
            .filter(|row| matches!(row, HelpRow::Shortcut { .. }))
            .count(),
    )
}

pub(crate) fn draw_help(frame: &mut Frame<'_>, app: &mut App, theme: &Theme) {
    let (selected, mut scroll, hover) = match &app.modal {
        Some(Modal::Help {
            selected,
            scroll,
            hover,
        }) => (*selected, *scroll, *hover),
        _ => return,
    };
    let area = centered_rect(
        72,
        34.min(frame.area().height.saturating_sub(2)),
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Keyboard Shortcuts ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let list_height = inner.height.saturating_sub(1) as usize;
    let exit_locked = app.exit_locked();
    let rows = help_rows(exit_locked);
    let display_selected = help_display_index(selected, exit_locked);
    if app.modal_free_scroll {
        scroll = scroll.min(rows.len().saturating_sub(list_height));
    } else {
        ensure_offset(&mut scroll, display_selected, list_height, rows.len());
    }
    let mut hits = Vec::new();
    let end = (scroll + list_height).min(rows.len());
    for (y, (display, row)) in (inner.y..inner.y.saturating_add(cells(list_height)))
        .zip(rows.into_iter().enumerate().take(end).skip(scroll))
    {
        match row {
            HelpRow::Section(title) => {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        (*title).to_owned(),
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    )))
                    .style(Style::default().bg(theme.panel_alt)),
                    Rect::new(inner.x, y, inner.width, 1),
                );
            }
            HelpRow::Spacer => {
                frame.render_widget(
                    Paragraph::new(" ").style(Style::default().bg(theme.panel)),
                    Rect::new(inner.x, y, inner.width, 1),
                );
            }
            HelpRow::Shortcut { keys, description } => {
                let Some(index) = help_shortcut_index_at(display, exit_locked) else {
                    continue;
                };
                let background = if index == selected {
                    theme.selected
                } else if hover == Some(index) {
                    theme.panel_alt
                } else {
                    theme.panel
                };
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            format!("{keys:<22}"),
                            Style::default().fg(theme.modified).bg(background),
                        ),
                        Span::styled(
                            (*description).to_owned(),
                            Style::default().fg(theme.text).bg(background),
                        ),
                    ]))
                    .style(Style::default().bg(background)),
                    Rect::new(inner.x, y, inner.width, 1),
                );
                hits.push(HelpHit {
                    area: Rect::new(inner.x, y, inner.width, 1),
                    index,
                });
            }
        }
    }
    if let Some(Modal::Help { scroll: stored, .. }) = &mut app.modal {
        *stored = scroll;
    }
    app.geometry.help_hits = hits;
    draw_modal_hint(frame, area, "j/k select · Esc close", theme);
}
