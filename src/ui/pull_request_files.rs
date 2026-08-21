#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::needless_pass_by_value,
    reason = "the caller has no use for the value afterwards"
)]
pub(super) fn draw_pull_request_section_tab(
    frame: &mut Frame<'_>,
    area: Rect,
    label: String,
    selected: bool,
    theme: &Theme,
) {
    frame.render_widget(
        Paragraph::new(truncate_end(&label, area.width as usize))
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.accent_soft
                    } else {
                        theme.panel
                    })
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        area,
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
pub(super) fn draw_pull_request_file_tree(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<SidebarHitArea> {
    if app.pull_request_files.is_empty() {
        let message = if app.document_loading || app.pull_request_progress.is_some() {
            "\n  Preparing local diff index…"
        } else {
            "\n  No changed files"
        };
        frame.render_widget(
            Paragraph::new(message).style(Style::default().fg(theme.muted)),
            area,
        );
        return Vec::new();
    }
    let row_count = app.pull_request_tree_entries().len();
    app.pull_request_tree_cursor = app
        .pull_request_tree_cursor
        .min(row_count.saturating_sub(1));
    app.sidebar_viewport(
        app.pull_request_tree_cursor,
        area.height as usize,
        row_count,
    );
    let rows = &app.pull_request_tree;
    let mut hits = Vec::new();
    for (offset, row) in rows
        .iter()
        .skip(app.sidebar_offset)
        .take(area.height as usize)
        .enumerate()
    {
        let row_index = app.sidebar_offset + offset;
        let y = area.y + cells(offset);
        let selected = row_index == app.pull_request_tree_cursor;
        let background = if selected {
            theme.selected
        } else {
            theme.panel
        };
        match row {
            PullRequestTreeEntry::Directory { path, label, depth } => {
                let background = if selected {
                    theme.selected
                } else {
                    theme.panel_alt
                };
                let indent_width = depth.saturating_mul(2).min(16);
                let available = (area.width as usize)
                    .saturating_sub(indent_width)
                    .saturating_sub(5);
                let icon = disclosure_glyph(!app.pull_request_directory_collapsed(path));
                frame.render_widget(
                    Paragraph::new(format!(
                        " {}{icon} {}/",
                        "  ".repeat((*depth).min(8)),
                        truncate_end(label, available),
                    ))
                    .style(
                        Style::default()
                            .fg(if selected { theme.text } else { theme.muted })
                            .bg(background)
                            .add_modifier(if selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                    Rect::new(area.x, y, area.width, 1),
                );
                hits.push(SidebarHitArea {
                    area: Rect::new(area.x, y, area.width, 1),
                    target: SidebarHit::PullRequestDirectory(path.clone()),
                });
            }
            PullRequestTreeEntry::File { depth, index } => {
                let Some(file) = app.pull_request_files.get(*index) else {
                    continue;
                };
                let indent_width = depth.saturating_mul(2).min(16);
                let name = file.path.file_name().map_or_else(
                    || file.path.to_string_lossy(),
                    |name| name.to_string_lossy(),
                );
                let available = (area.width as usize)
                    .saturating_sub(indent_width)
                    .saturating_sub(9);
                let line = Line::from(vec![
                    Span::raw(format!(
                        " {}{}",
                        "  ".repeat((*depth).min(8)),
                        if selected { "• " } else { "  " },
                    )),
                    file_icon_span(&file.path, theme),
                    Span::raw(" "),
                    Span::styled(
                        truncate_end(&name, available),
                        Style::default().fg(theme.text).add_modifier(if selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    ),
                ]);
                frame.render_widget(
                    Paragraph::new(line).style(Style::default().bg(background)),
                    Rect::new(area.x, y, area.width.saturating_sub(3), 1),
                );
                frame.render_widget(
                    Paragraph::new(pull_request_file_status_code(file.status))
                        .alignment(Alignment::Right)
                        .style(
                            Style::default()
                                .fg(pull_request_file_status_color(file.status, theme))
                                .bg(background)
                                .add_modifier(Modifier::BOLD),
                        ),
                    Rect::new(area.right().saturating_sub(3), y, 2, 1),
                );
                hits.push(SidebarHitArea {
                    area: Rect::new(area.x, y, area.width, 1),
                    target: SidebarHit::PullRequestFile(*index),
                });
            }
        }
    }
    draw_scrollbar(frame, area, app.sidebar_offset, rows.len(), theme);
    hits
}

pub(super) const fn pull_request_file_status_code(status: PullRequestFileStatus) -> &'static str {
    match status {
        PullRequestFileStatus::Added => "A",
        PullRequestFileStatus::Modified => "M",
        PullRequestFileStatus::Deleted => "D",
        PullRequestFileStatus::Renamed => "R",
        PullRequestFileStatus::Copied => "C",
        PullRequestFileStatus::TypeChanged => "T",
        PullRequestFileStatus::Unmerged => "U",
        PullRequestFileStatus::Unknown => "?",
    }
}

pub(super) const fn pull_request_file_status_color(
    status: PullRequestFileStatus,
    theme: &Theme,
) -> Color {
    match status {
        PullRequestFileStatus::Added => theme.added,
        PullRequestFileStatus::Deleted => theme.removed,
        PullRequestFileStatus::Renamed
        | PullRequestFileStatus::Copied
        | PullRequestFileStatus::Modified
        | PullRequestFileStatus::TypeChanged => theme.modified,
        PullRequestFileStatus::Unmerged => theme.conflict,
        PullRequestFileStatus::Unknown => theme.muted,
    }
}

/// The overview sidebar is the pull request itself on row zero, then status
/// sections and their checks, so one list carries both the way back to the
/// conversation and the way into any run's log.
pub(super) fn draw_pull_request_check_list(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<SidebarHitArea> {
    if area.height == 0 {
        return Vec::new();
    }
    let rows = app.check_list_rows();
    let selected_row = rows
        .iter()
        .position(|row| check_list_row_selected(app, row))
        .unwrap_or_default();
    app.sidebar_viewport(selected_row, area.height as usize, rows.len());

    let mut hits = Vec::new();
    let end = (app.sidebar_offset + area.height as usize).min(rows.len());
    for (y, row) in (area.y..area.bottom()).zip(rows.iter().take(end).skip(app.sidebar_offset)) {
        let row_area = Rect::new(area.x, y, area.width, 1);
        match row {
            CheckListRow::Spacer => {
                let rule = "─".repeat((area.width as usize).saturating_sub(4));
                frame.render_widget(
                    Paragraph::new(format!("  {rule}"))
                        .style(Style::default().fg(theme.border).bg(theme.panel)),
                    row_area,
                );
            }
            CheckListRow::Conversation => {
                hits.push(draw_check_list_conversation(frame, row_area, app, theme));
            }
            CheckListRow::Heading => {
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            "CHECKS",
                            Style::default()
                                .fg(theme.accent)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("  {}", app.pull_request_checks.len()),
                            Style::default().fg(theme.muted),
                        ),
                    ]))
                    .style(Style::default().bg(theme.panel_alt)),
                    row_area,
                );
            }
            CheckListRow::Section {
                section,
                count,
                collapsed,
            } => {
                let selected = app.selected_check_section == Some(*section);
                let background = if selected {
                    theme.selected
                } else {
                    theme.panel_alt
                };
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            disclosure_prefix(!*collapsed),
                            Style::default().fg(theme.muted),
                        ),
                        Span::styled(
                            section.label().to_uppercase(),
                            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("  {count}"), Style::default().fg(theme.muted)),
                    ]))
                    .style(Style::default().bg(background)),
                    row_area,
                );
                hits.push(SidebarHitArea {
                    area: row_area,
                    target: SidebarHit::PullRequestCheckSection(*section),
                });
            }
            CheckListRow::Check { index } => {
                if let Some(hit) = draw_check_list_check(frame, row_area, app, theme, *index) {
                    hits.push(hit);
                }
            }
        }
    }
    draw_scrollbar(frame, area, app.sidebar_offset, rows.len(), theme);

    if app.pull_request_checks.is_empty() && area.height > 1 {
        let message = if app.pull_request_checks_loading {
            "  Loading checks…".to_owned()
        } else if let Some(error) = app.pull_request_checks_error.as_deref() {
            format!("  {error}")
        } else {
            "  No checks reported".to_owned()
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(Style::default().fg(theme.muted).bg(theme.panel))
                .wrap(Wrap { trim: true }),
            Rect::new(area.x, area.y + 1, area.width, area.height - 1),
        );
    }
    hits
}

pub(super) fn check_list_row_selected(app: &App, row: &CheckListRow) -> bool {
    match row {
        CheckListRow::Conversation => {
            app.selected_check_section.is_none() && app.pull_request_check_cursor.is_none()
        }
        CheckListRow::Section { section, .. } => app.selected_check_section == Some(*section),
        CheckListRow::Check { index } => {
            app.selected_check_section.is_none() && app.pull_request_check_cursor == Some(*index)
        }
        CheckListRow::Heading | CheckListRow::Spacer => false,
    }
}

pub(super) fn draw_check_list_conversation(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
) -> SidebarHitArea {
    let selected = app.selected_check_section.is_none() && app.pull_request_check_cursor.is_none();
    let background = if selected {
        theme.selected
    } else {
        theme.panel
    };
    let marker = Span::styled(
        if selected { " › " } else { "   " },
        Style::default().fg(theme.accent),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            marker,
            Span::styled(
                "Conversation",
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                conversation_row_suffix(app),
                Style::default().fg(theme.muted),
            ),
        ]))
        .style(Style::default().bg(background)),
        area,
    );
    SidebarHitArea {
        area,
        target: SidebarHit::PullRequestConversation,
    }
}

pub(super) fn draw_check_list_check(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
    index: usize,
) -> Option<SidebarHitArea> {
    let check = app.pull_request_checks.get(index)?;
    let selected =
        app.selected_check_section.is_none() && app.pull_request_check_cursor == Some(index);
    let background = if selected {
        theme.selected
    } else {
        theme.panel
    };
    let marker = Span::styled(
        if selected { " › " } else { "   " },
        Style::default().fg(theme.accent),
    );
    let (icon, color) = pull_request_check_icon(check.status, theme);
    let workflow = if check.workflow.is_empty() {
        String::new()
    } else {
        format!("  {}", check.workflow)
    };
    let reserved = 6 + workflow.width();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            marker,
            Span::styled(format!("{icon} "), Style::default().fg(color)),
            Span::styled(
                truncate_end(&check.name, (area.width as usize).saturating_sub(reserved)),
                Style::default().fg(theme.text),
            ),
            Span::styled(workflow, Style::default().fg(theme.muted)),
        ]))
        .style(Style::default().bg(background)),
        area,
    );
    Some(SidebarHitArea {
        area,
        target: SidebarHit::PullRequestCheck(index),
    })
}

pub(super) fn conversation_row_suffix(app: &App) -> String {
    if app.pull_request_conversation_error.is_some() {
        return "  ⚠".to_owned();
    }
    let comments = app.pull_request_conversation.comment_count();
    let mut suffix = if comments == 0 {
        String::new()
    } else {
        format!("  {comments}")
    };
    if app.pull_request_conversation_loading {
        suffix.push_str("  · loading");
    }
    suffix
}

pub(super) const fn pull_request_check_icon(
    status: PullRequestCheckStatus,
    theme: &Theme,
) -> (&'static str, Color) {
    match status {
        PullRequestCheckStatus::Passed => ("✓", theme.success),
        PullRequestCheckStatus::Failed => ("×", theme.error),
        PullRequestCheckStatus::Pending => ("◌", theme.accent),
        PullRequestCheckStatus::Skipped => ("−", theme.muted),
        PullRequestCheckStatus::Cancelled => ("■", theme.removed),
        PullRequestCheckStatus::Unknown => ("?", theme.muted),
    }
}
