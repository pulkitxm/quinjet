use super::*;

pub(super) fn draw_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> (Vec<SidebarHitArea>, Vec<ScmActionHit>) {
    match app.view {
        View::Changes => draw_changes_sidebar(frame, area, app, theme),
        View::History => (
            draw_history_sidebar(frame, area, app, theme, link_hits),
            Vec::new(),
        ),
        View::PullRequests => draw_pull_requests_sidebar(frame, area, app, theme, link_hits),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
pub(super) fn draw_changes_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> (Vec<SidebarHitArea>, Vec<ScmActionHit>) {
    let block = panel_block(
        if app.filter.is_empty() {
            format!(" Changes  {} ", app.status.changes.len())
        } else {
            format!(" Changes  /{} ", app.filter)
        },
        app.focus == Focus::Sidebar && app.modal.is_none(),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return (Vec::new(), Vec::new());
    }

    let checked_count = app.checked_change_count();
    let controls_height = if inner.height == 0 {
        0
    } else if checked_count == 0 || inner.height == 1 {
        1
    } else {
        2
    };
    let list_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(controls_height),
    );
    let visible = app.visible_change_indices();
    let rows = app.change_rows();
    let row_count = rows.len();
    let height = list_area.height as usize;
    let selected_row = rows
        .iter()
        .position(|row| match row {
            ChangeRow::Section { section, .. } => app.selected_change_section == Some(*section),
            ChangeRow::Change { cursor, .. } => {
                app.selected_change_section.is_none() && *cursor == app.change_cursor
            }
            ChangeRow::Spacer => false,
        })
        .unwrap_or_default();
    app.sidebar_viewport(selected_row, height, row_count);
    let mut hits = Vec::new();
    let mut action_hits = Vec::new();
    let end = (app.sidebar_offset + height).min(rows.len());
    for (y, row) in
        (list_area.y..list_area.bottom()).zip(rows.iter().take(end).skip(app.sidebar_offset))
    {
        match row {
            ChangeRow::Spacer => {
                frame.render_widget(
                    Paragraph::new(" ").style(Style::default().bg(theme.panel)),
                    Rect::new(list_area.x, y, list_area.width, 1),
                );
            }
            ChangeRow::Section {
                section,
                count,
                collapsed,
            } => {
                let selected = app.selected_change_section == Some(*section);
                let background = if selected {
                    theme.selected
                } else {
                    theme.panel_alt
                };
                let check_label = app.section_check_label(*section);
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            disclosure_prefix(!collapsed),
                            Style::default().fg(theme.muted),
                        ),
                        Span::styled(
                            check_label,
                            Style::default().fg(if check_label == "[ ]" {
                                theme.muted
                            } else {
                                theme.accent
                            }),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            section.label().to_uppercase(),
                            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("  {count}"), Style::default().fg(theme.muted)),
                    ]))
                    .style(Style::default().bg(background)),
                    Rect::new(list_area.x, y, list_area.width, 1),
                );
                action_hits.push(ScmActionHit {
                    area: Rect::new(list_area.x.saturating_add(3), y, 3, 1),
                    action: ScmAction::ToggleCheckSection(*section),
                });
                let (label, action) = match section {
                    ChangeSection::Staged => ("[−]", ScmAction::UnstageSection(*section)),
                    ChangeSection::Conflict | ChangeSection::Unstaged => {
                        ("[+]", ScmAction::StageSection(*section))
                    }
                };
                let action_area = Rect::new(list_area.right().saturating_sub(4), y, 4, 1);
                frame.render_widget(
                    Paragraph::new(label)
                        .alignment(Alignment::Right)
                        .style(Style::default().fg(theme.accent).bg(background)),
                    action_area,
                );
                action_hits.push(ScmActionHit {
                    area: action_area,
                    action,
                });
                hits.push(SidebarHitArea {
                    area: Rect::new(list_area.x, y, list_area.width, 1),
                    target: SidebarHit::ChangeSection(*section),
                });
            }
            ChangeRow::Change { index, cursor, .. } => {
                let Some(change) = app.status.changes.get(*index) else {
                    continue;
                };
                let selected =
                    app.selected_change_section.is_none() && *cursor == app.change_cursor;
                let row_style = if selected {
                    Style::default().bg(theme.selected)
                } else {
                    Style::default().bg(theme.panel)
                };
                let checked = app.checked_change_paths.contains(&change.path);
                let path = change.parent_path();
                let available = list_area.width.saturating_sub(17) as usize;
                let name = truncate_middle(
                    &change.file_name(),
                    available.saturating_sub(path.width() + 1),
                );
                let check_label = if checked { "[x]" } else { "[ ]" };
                let line = Line::from(vec![
                    Span::styled(
                        if selected { "•" } else { " " },
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(
                        check_label,
                        Style::default().fg(if checked { theme.accent } else { theme.muted }),
                    ),
                    Span::raw(" "),
                    file_icon_span(&change.path, theme),
                    Span::raw(" "),
                    Span::styled(
                        name,
                        Style::default().fg(theme.text).add_modifier(if selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    ),
                    Span::styled(
                        if path.is_empty() {
                            String::new()
                        } else {
                            format!("  {path}")
                        },
                        Style::default().fg(theme.muted),
                    ),
                ]);
                frame.render_widget(
                    Paragraph::new(line).style(row_style),
                    Rect::new(list_area.x, y, list_area.width.saturating_sub(7), 1),
                );
                action_hits.push(ScmActionHit {
                    area: Rect::new(list_area.x.saturating_add(1), y, 3, 1),
                    action: ScmAction::ToggleCheck(*index),
                });
                let (action_label, action) = match change.area {
                    ChangeArea::Staged => ("[−]", ScmAction::Unstage(*index)),
                    ChangeArea::Conflict => ("[!]", ScmAction::Resolve(*index)),
                    ChangeArea::Unstaged => ("[+]", ScmAction::Stage(*index)),
                };
                let background = if selected {
                    theme.selected
                } else {
                    theme.panel
                };
                let status_area = Rect::new(list_area.right().saturating_sub(7), y, 3, 1);
                frame.render_widget(
                    Paragraph::new(change.status.code())
                        .alignment(Alignment::Right)
                        .style(
                            Style::default()
                                .fg(status_color(change.status, theme))
                                .bg(background)
                                .add_modifier(Modifier::BOLD),
                        ),
                    status_area,
                );
                let action_area = Rect::new(list_area.right().saturating_sub(4), y, 4, 1);
                frame.render_widget(
                    Paragraph::new(action_label)
                        .alignment(Alignment::Right)
                        .style(
                            Style::default()
                                .fg(theme.accent)
                                .bg(background)
                                .add_modifier(Modifier::BOLD),
                        ),
                    action_area,
                );
                action_hits.push(ScmActionHit {
                    area: action_area,
                    action,
                });
                hits.push(SidebarHitArea {
                    area: Rect::new(list_area.x, y, list_area.width, 1),
                    target: SidebarHit::Change(*index),
                });
            }
        }
    }

    draw_scrollbar(frame, list_area, app.sidebar_offset, rows.len(), theme);

    if visible.is_empty() {
        let message = if app.status.changes.is_empty() {
            "\n  ✓ Working tree clean\n\n  No pending changes"
        } else {
            "\n  No changes match this filter"
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(Style::default().fg(if app.status.changes.is_empty() {
                    theme.success
                } else {
                    theme.muted
                }))
                .wrap(Wrap { trim: false }),
            list_area,
        );
    }

    let controls_y = list_area.bottom();
    if controls_height >= 1 {
        let arrow_width = 3.min(inner.width);
        let button_width = inner.width.saturating_sub(arrow_width);
        let primary_label = if app.primary_is_stash() {
            format!("Stash ({checked_count})")
        } else {
            "Commit".to_owned()
        };
        let revert_label = (controls_height == 2).then(|| format!("Revert ({checked_count})"));
        let indent = toolbar_indent(
            button_width,
            revert_label
                .iter()
                .chain(std::iter::once(&primary_label))
                .map(|label| label.width())
                .max()
                .unwrap_or_default(),
        );
        let primary_row = Rect::new(
            inner.x,
            controls_y.saturating_add(controls_height.saturating_sub(1)),
            inner.width,
            1,
        );
        let button_area = |row: Rect| Rect::new(row.x, row.y, button_width, 1);
        let revert_row = revert_label.map(|label| {
            let row = Rect::new(inner.x, controls_y, inner.width, 1);
            let area = button_area(row);
            frame.render_widget(
                Paragraph::new(
                    " ".repeat(indent)
                        + &truncate_middle(&label, (button_width as usize).saturating_sub(indent)),
                )
                .style(
                    Style::default()
                        .fg(theme.error)
                        .bg(theme.removed_background)
                        .add_modifier(Modifier::BOLD),
                ),
                area,
            );
            action_hits.push(ScmActionHit {
                area,
                action: ScmAction::RevertChecked,
            });
            row
        });
        let primary_color = if app.primary_is_stash() {
            theme.modified
        } else {
            theme.accent
        };
        let label_area = button_area(primary_row);
        let arrow_area = Rect::new(
            primary_row.x.saturating_add(button_width),
            primary_row.y,
            arrow_width,
            1,
        );
        frame.render_widget(
            Paragraph::new(
                " ".repeat(indent)
                    + &truncate_middle(
                        &primary_label,
                        (button_width as usize).saturating_sub(indent),
                    ),
            )
            .style(
                Style::default()
                    .fg(primary_color)
                    .bg(theme.panel_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            label_area,
        );
        frame.render_widget(
            Paragraph::new("▶")
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
            arrow_area,
        );
        action_hits.push(ScmActionHit {
            area: label_area,
            action: ScmAction::Primary,
        });
        action_hits.push(ScmActionHit {
            area: arrow_area,
            action: ScmAction::ToggleMenu,
        });
        if app.scm_menu_open {
            let items = app
                .scm_menu_items()
                .into_iter()
                .map(|item| {
                    let label = app.scm_menu_label(item);
                    (item, label)
                })
                .collect::<Vec<_>>();
            draw_scm_menu(
                frame,
                revert_row.unwrap_or(primary_row),
                app.scm_menu_selected,
                &items,
                theme,
                &mut action_hits,
            );
        }
    }
    (hits, action_hits)
}

#[expect(
    clippy::integer_division,
    reason = "an odd amount of free space intentionally leaves the extra column on the right"
)]
pub(super) fn toolbar_indent(button_width: u16, widest_label: usize) -> usize {
    let room = (button_width as usize).saturating_sub(widest_label);
    if room == 0 { 0 } else { (room / 2).max(1) }
}

pub(super) fn overflow_menu_area(anchor: Rect, width: usize, item_count: usize) -> Rect {
    let width = u16::try_from(width).unwrap_or(24).min(anchor.width.max(1));
    let item_count = u16::try_from(item_count).unwrap_or(1);
    let height = item_count.saturating_add(2);
    let x = anchor.x.saturating_add(anchor.width.saturating_sub(width));
    let y = anchor.y.saturating_sub(height);
    Rect::new(x, y, width, height)
}

pub(super) fn draw_scm_menu(
    frame: &mut Frame<'_>,
    anchor: Rect,
    selected: usize,
    items: &[(ScmMenuItem, String)],
    theme: &Theme,
    action_hits: &mut Vec<ScmActionHit>,
) {
    if items.is_empty() {
        return;
    }
    let width = items
        .iter()
        .map(|(_, label)| label.width())
        .max()
        .unwrap_or(12)
        .saturating_add(4);
    let area = overflow_menu_area(anchor, width, items.len());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focus))
        .style(Style::default().bg(theme.panel));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    for (index, (item, label)) in items.iter().enumerate() {
        let active = index == selected;
        let row = Rect::new(
            inner.x,
            inner.y.saturating_add(u16::try_from(index).unwrap_or(0)),
            inner.width,
            1,
        );
        frame.render_widget(
            Paragraph::new(format!(" {label} ")).style(if active {
                Style::default().fg(theme.text).bg(theme.selected)
            } else {
                Style::default().fg(theme.text).bg(theme.panel)
            }),
            row,
        );
        action_hits.push(ScmActionHit {
            area: row,
            action: ScmAction::Menu(*item),
        });
    }
}
