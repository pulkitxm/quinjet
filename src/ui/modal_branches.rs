#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::too_many_arguments,
    reason = "the branch picker needs the live worktree list from the session"
)]
pub(super) fn draw_branches(
    frame: &mut Frame<'_>,
    items: &[Branch],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    app: &App,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let height = frame.area().height.saturating_sub(8).min(25);
    let area = centered_rect(
        frame.area().width.saturating_sub(12).min(76),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Branches ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );

    let visible = App::filtered_branches(items, &query.value);
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    if loading {
        frame.render_widget(
            Paragraph::new("Loading branches…").style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let offset = list.offset(selected, list_area.height as usize, visible.len());
        let lines = visible
            .iter()
            .skip(offset)
            .take(list_area.height as usize)
            .enumerate()
            .filter_map(|(visible_offset, index)| {
                let branch = items.get(*index)?;
                let cursor = offset + visible_offset;
                let style = if cursor == selected {
                    Style::default()
                        .bg(theme.selected)
                        .fg(theme.text)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text)
                };
                Some(Line::from(vec![
                    Span::styled(
                        if branch.current { " ● " } else { "   " },
                        Style::default()
                            .fg(if branch.current {
                                theme.success
                            } else {
                                theme.muted
                            })
                            .bg(style.bg.unwrap_or(theme.panel)),
                    ),
                    Span::styled(
                        truncate_middle(&branch.name, list_area.width.saturating_sub(30) as usize),
                        style,
                    ),
                    Span::styled(
                        {
                            let mut meta = format!(
                                "  {}  {}",
                                branch.short_id,
                                format_relative_timestamp(&branch.relative_date)
                            );
                            if let Some(path) = app.worktree_path_for_branch(&branch.name) {
                                meta.push_str("  ");
                                meta.push_str(&truncate_middle(&path.display().to_string(), 24));
                            }
                            meta
                        },
                        Style::default()
                            .fg(theme.muted)
                            .bg(style.bg.unwrap_or(theme.panel)),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        for index in 0..lines.len() {
            list.hit(
                Rect::new(
                    list_area.x,
                    list_area.y.saturating_add(cells(index)),
                    list_area.width,
                    1,
                ),
                offset.saturating_add(index),
            );
        }
        frame.render_widget(Paragraph::new(lines), list_area);
    }
    draw_modal_hint(
        frame,
        area,
        "Enter switch   Ctrl+n new   F2/Ctrl+r rename   Delete delete   Esc close",
        theme,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "the picker receives its modal state, list geometry, and palette"
)]
pub(super) fn draw_history_branches(
    frame: &mut Frame<'_>,
    items: &[HistoryBranch],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let height = frame.area().height.saturating_sub(8).min(25);
    let area = centered_rect(
        frame.area().width.saturating_sub(12).min(82),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" View Branch History — no checkout ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    if loading {
        frame.render_widget(
            Paragraph::new("Loading local and remote-tracking branches…")
                .style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let visible = App::filtered_history_branches(items, &query.value);
        let offset = list.offset(selected, list_area.height as usize, visible.len());
        let lines = visible
            .iter()
            .skip(offset)
            .take(list_area.height as usize)
            .enumerate()
            .filter_map(|(visible_offset, index)| {
                let branch = items.get(*index)?;
                let active = offset + visible_offset == selected;
                let background = if active { theme.selected } else { theme.panel };
                Some(Line::from(vec![
                    Span::styled(
                        if branch.current { " ● " } else { "   " },
                        Style::default()
                            .fg(if branch.current {
                                theme.success
                            } else {
                                theme.muted
                            })
                            .bg(background),
                    ),
                    Span::styled(
                        truncate_middle(&branch.name, list_area.width.saturating_sub(34) as usize),
                        Style::default()
                            .fg(theme.text)
                            .bg(background)
                            .add_modifier(if active {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                    Span::styled(
                        format!(
                            "  {}  {}  {}",
                            if branch.remote { "remote" } else { "local" },
                            branch.short_id,
                            format_relative_timestamp(&branch.relative_date)
                        ),
                        Style::default().fg(theme.muted).bg(background),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        for index in 0..lines.len() {
            list.hit(
                Rect::new(
                    list_area.x,
                    list_area.y.saturating_add(cells(index)),
                    list_area.width,
                    1,
                ),
                offset.saturating_add(index),
            );
        }
        frame.render_widget(Paragraph::new(lines), list_area);
    }
    draw_modal_hint(
        frame,
        area,
        "Enter view history (HEAD and worktree stay unchanged)   Esc close",
        theme,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "the picker receives its modal state, list geometry, and palette"
)]
pub(super) fn draw_compare_branches(
    frame: &mut Frame<'_>,
    items: &[HistoryBranch],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let height = frame.area().height.saturating_sub(8).min(25);
    let area = centered_rect(
        frame.area().width.saturating_sub(12).min(82),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Compare Current Branch With… ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    if loading {
        frame.render_widget(
            Paragraph::new("Loading local and remote-tracking branches…")
                .style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let visible = App::filtered_history_branches(items, &query.value)
            .into_iter()
            .filter(|index| items.get(*index).is_some_and(|item| !item.current))
            .collect::<Vec<_>>();
        let offset = list.offset(selected, list_area.height as usize, visible.len());
        let lines = visible
            .iter()
            .skip(offset)
            .take(list_area.height as usize)
            .enumerate()
            .filter_map(|(visible_offset, index)| {
                let branch = items.get(*index)?;
                let active = offset + visible_offset == selected;
                let background = if active { theme.selected } else { theme.panel };
                Some(Line::from(vec![
                    Span::styled(
                        if active { " › " } else { "   " },
                        Style::default().fg(theme.accent).bg(background),
                    ),
                    Span::styled(
                        truncate_middle(&branch.name, list_area.width.saturating_sub(34) as usize),
                        Style::default()
                            .fg(theme.text)
                            .bg(background)
                            .add_modifier(if active {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                    Span::styled(
                        format!(
                            "  {}  {}  {}",
                            if branch.remote { "remote" } else { "local" },
                            branch.short_id,
                            format_relative_timestamp(&branch.relative_date)
                        ),
                        Style::default().fg(theme.muted).bg(background),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        for index in 0..lines.len() {
            list.hit(
                Rect::new(
                    list_area.x,
                    list_area.y.saturating_add(cells(index)),
                    list_area.width,
                    1,
                ),
                offset.saturating_add(index),
            );
        }
        frame.render_widget(Paragraph::new(lines), list_area);
        if visible.is_empty() {
            frame.render_widget(
                Paragraph::new("No other branches match this filter")
                    .style(Style::default().fg(theme.muted)),
                list_area,
            );
        }
    }
    draw_modal_hint(
        frame,
        area,
        "Enter calculate diff against HEAD   Esc close",
        theme,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "the picker receives its modal state, list geometry, and palette"
)]
pub(super) fn draw_stashes(
    frame: &mut Frame<'_>,
    items: &[Stash],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let height = frame.area().height.saturating_sub(6).min(27);
    let area = centered_rect(
        frame.area().width.saturating_sub(10).min(88),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Stashes ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(5),
    );
    if loading {
        frame.render_widget(
            Paragraph::new("Loading stashes…").style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let visible = App::filtered_stashes(items, &query.value);
        let offset = list.offset(selected, list_area.height as usize, visible.len());
        let lines = visible
            .iter()
            .skip(offset)
            .take(list_area.height as usize)
            .enumerate()
            .filter_map(|(visible_offset, index)| {
                let stash = items.get(*index)?;
                let active = offset + visible_offset == selected;
                let background = if active { theme.selected } else { theme.panel };
                let branch = if stash.branch.is_empty() {
                    String::new()
                } else {
                    format!(" on {}", stash.branch)
                };
                Some(Line::from(vec![
                    Span::styled(
                        if active { " › " } else { "   " },
                        Style::default().fg(theme.accent).bg(background),
                    ),
                    Span::styled(
                        format!("{}  ", stash.reference),
                        Style::default()
                            .fg(theme.modified)
                            .bg(background)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        truncate_middle(
                            &stash.message,
                            list_area.width.saturating_sub(40) as usize,
                        ),
                        Style::default().fg(theme.text).bg(background),
                    ),
                    Span::styled(
                        format!(
                            "{branch}  {}  {}",
                            stash.short_id,
                            format_relative_timestamp(&stash.relative_date)
                        ),
                        Style::default().fg(theme.muted).bg(background),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        for index in 0..lines.len() {
            list.hit(
                Rect::new(
                    list_area.x,
                    list_area.y.saturating_add(cells(index)),
                    list_area.width,
                    1,
                ),
                offset.saturating_add(index),
            );
        }
        frame.render_widget(Paragraph::new(lines), list_area);
        if visible.is_empty() {
            frame.render_widget(
                Paragraph::new(if items.is_empty() {
                    "No stashes — Ctrl+n creates one"
                } else {
                    "No stashes match this filter"
                })
                .style(Style::default().fg(theme.muted)),
                list_area,
            );
        }
    }
    draw_modal_hint(
        frame,
        area,
        "Enter preview  Ctrl+n new  Ctrl+u +untracked  Ctrl+s staged  Alt+a apply  Alt+p pop  Del drop",
        theme,
    );
}
