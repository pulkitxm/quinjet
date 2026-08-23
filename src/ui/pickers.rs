#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::too_many_lines,
    reason = "the picker renders headings and worktree rows in one pass"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "the picker receives one value for each modal field plus its palette"
)]
pub(crate) fn draw_projects(
    frame: &mut Frame<'_>,
    collapse_hits: &mut Vec<(Rect, std::path::PathBuf)>,
    groups: &[ProjectGroup],
    selected: usize,
    query: &crate::app::TextBuffer,
    collapsed: &HashSet<std::path::PathBuf>,
    loading: bool,
    opening: Option<&Path>,
    mode: ProjectOpenMode,
    ssh: Option<&SshContext>,
    machine_focus: Option<usize>,
    list: &mut ModalList<'_>,
    theme: &Theme,
) -> Vec<(Rect, usize)> {
    let height = frame.area().height.saturating_sub(6).min(28);
    let area = centered_rect(
        frame.area().width.saturating_sub(10).min(88),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let title = match mode {
        ProjectOpenMode::Initial => " Open a project ",
        ProjectOpenMode::CurrentTab => " Switch project ",
        ProjectOpenMode::NewTab => " Open in new tab ",
    };
    let block = modal_block(title, theme);
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
    if machine_focus.is_none() {
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
    }
    let machine_hits = ssh.map_or_else(Vec::new, |context| {
        modal_ssh::draw_project_machines(
            frame,
            Rect::new(inner.x, inner.y.saturating_add(2), inner.width, 1),
            context,
            machine_focus,
            theme,
        )
    });
    let machine_rows = if ssh.is_some() { 2 } else { 0 };
    let list_area = Rect::new(
        inner.x,
        inner.y.saturating_add(2 + machine_rows),
        inner.width,
        inner.height.saturating_sub(5 + machine_rows),
    );
    if let Some(path) = opening {
        modal_ssh::draw_project_opening(frame, path, list_area, theme);
    } else if loading {
        frame.render_widget(
            Paragraph::new("Loading projects…").style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let rows = App::filtered_project_rows(groups, &query.value, collapsed);
        let offset = list.offset(selected, list_area.height as usize, rows.len());
        let visible_lines = rows
            .iter()
            .enumerate()
            .skip(offset)
            .take(list_area.height as usize)
            .filter_map(|(index, row)| match row {
                ProjectRow::Group(group_index) => {
                    let group = groups.get(*group_index)?;
                    let expanded =
                        !collapsed.contains(&group.common_dir) || !query.value.is_empty();
                    Some((
                        project_header_line(
                            group,
                            expanded,
                            index == selected,
                            list_area.width as usize,
                            theme,
                        ),
                        Some(group.common_dir.clone()),
                        index,
                    ))
                }
                ProjectRow::Worktree {
                    group_index,
                    tree_index,
                } => {
                    let tree = groups.get(*group_index)?.worktrees.get(*tree_index)?;
                    Some((
                        project_worktree_line(
                            tree,
                            index == selected,
                            list_area.width as usize,
                            theme,
                        ),
                        None,
                        index,
                    ))
                }
            })
            .collect::<Vec<_>>();
        if visible_lines.is_empty() {
            let empty = if groups.is_empty() && mode == ProjectOpenMode::Initial {
                "No recent projects. Press Ctrl+O to enter a repository path."
            } else {
                "No projects match this filter"
            };
            frame.render_widget(
                Paragraph::new(empty).style(Style::default().fg(theme.muted)),
                list_area,
            );
        } else {
            collapse_hits.extend(visible_lines.iter().enumerate().filter_map(
                |(line_index, (_, common_dir, _))| {
                    common_dir.as_ref().map(|common_dir| {
                        (
                            Rect::new(
                                list_area.x.saturating_add(1),
                                list_area.y.saturating_add(cells(line_index)),
                                3,
                                1,
                            ),
                            common_dir.clone(),
                        )
                    })
                },
            ));
            for (line_index, (_, _, index)) in visible_lines.iter().enumerate() {
                list.hit(
                    Rect::new(
                        list_area.x,
                        list_area.y.saturating_add(cells(line_index)),
                        list_area.width,
                        1,
                    ),
                    *index,
                );
            }
            frame.render_widget(
                Paragraph::new(
                    visible_lines
                        .into_iter()
                        .map(|(line, ..)| line)
                        .collect::<Vec<_>>(),
                ),
                list_area,
            );
        }
    }
    let fold_action = if App::all_project_groups_expanded(groups, collapsed) {
        "collapse all"
    } else {
        "expand all"
    };
    let selected_group = matches!(
        App::filtered_project_rows(groups, &query.value, collapsed).get(selected),
        Some(ProjectRow::Group(_))
    );
    let hint = if opening.is_some() {
        "Opening project…".to_owned()
    } else if machine_focus.is_some() {
        let escape = if mode == ProjectOpenMode::Initial {
            "quit"
        } else {
            "close"
        };
        format!("←/→ choose machine   Enter switch   Tab projects   Esc {escape}")
    } else {
        let machines = ssh.map_or("", |_| "   Tab machines");
        if selected_group {
            let escape = if mode == ProjectOpenMode::Initial {
                "quit"
            } else {
                "close"
            };
            format!(
                "Enter/Space fold   ← collapse   → expand{machines}   Ctrl+E {fold_action}   Esc {escape}"
            )
        } else {
            match mode {
                ProjectOpenMode::Initial => {
                    format!("Enter open{machines}   Ctrl+E {fold_action}   Ctrl+O path   Esc quit")
                }
                ProjectOpenMode::CurrentTab => {
                    format!(
                        "Enter switch tab{machines}   Ctrl+E {fold_action}   Delete forget   Esc close"
                    )
                }
                ProjectOpenMode::NewTab => {
                    format!(
                        "Enter new tab{machines}   Ctrl+E {fold_action}   Delete forget   Esc close"
                    )
                }
            }
        }
    };
    draw_modal_hint(frame, area, &hint, theme);
    machine_hits
}

#[expect(
    clippy::too_many_arguments,
    reason = "the picker receives its modal state, list geometry, and palette"
)]
pub(super) fn draw_pull_request_repositories(
    frame: &mut Frame<'_>,
    items: &[GitHubRepository],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let height = (cells(items.len()) + 7)
        .min(frame.area().height.saturating_sub(8))
        .max(10);
    let area = centered_rect(
        frame.area().width.saturating_sub(12).min(82),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Pull Request Repository ", theme);
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
    let visible = App::filtered_github_repositories(items, &query.value);
    let offset = list.offset(selected, list_area.height as usize, visible.len());
    let lines = visible
        .iter()
        .skip(offset)
        .take(list_area.height as usize)
        .enumerate()
        .filter_map(|(visible_offset, index)| {
            let repository = items.get(*index)?;
            let active = offset + visible_offset == selected;
            let background = if active { theme.selected } else { theme.panel };
            let remotes = if repository.remotes.is_empty() {
                "inferred".to_owned()
            } else {
                repository.remotes.join(", ")
            };
            Some(Line::from(vec![
                Span::styled(
                    if active { " › " } else { "   " },
                    Style::default().fg(theme.accent).bg(background),
                ),
                Span::styled(
                    truncate_middle(
                        &repository.display_name(),
                        list_area.width.saturating_sub(24) as usize,
                    ),
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
                    format!("  remote {remotes}"),
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
    if loading {
        frame.render_widget(
            Paragraph::new("Discovering GitHub repositories from configured remotes…")
                .style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        frame.render_widget(Paragraph::new(lines), list_area);
    }
    draw_modal_hint(
        frame,
        area,
        if loading {
            "Discovering repositories…   Esc close"
        } else {
            "Enter select repository and reopen only the entered PR   Esc close"
        },
        theme,
    );
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
pub(super) fn draw_palette(
    frame: &mut Frame<'_>,
    app: &App,
    query: &crate::app::TextBuffer,
    selected: usize,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let commands = app.palette_commands(&query.value);
    let height = (cells(commands.len()) + 6)
        .min(frame.area().height.saturating_sub(6))
        .max(8);
    let area = Rect::new(
        frame.area().x
            + (frame
                .area()
                .width
                .saturating_sub(76.min(frame.area().width.saturating_sub(8))))
                / 2,
        frame.area().y + 3,
        76.min(frame.area().width.saturating_sub(8)),
        height,
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Command Palette ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" > ", Style::default().fg(theme.accent)),
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
        inner.height.saturating_sub(2),
    );
    let offset = list.offset(selected, list_area.height as usize, commands.len());
    let lines = commands
        .iter()
        .skip(offset)
        .take(list_area.height as usize)
        .enumerate()
        .map(|(index, command)| palette_line(*command, offset + index == selected, theme))
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

pub(super) fn palette_line(
    command: PaletteCommand,
    selected: bool,
    theme: &Theme,
) -> Line<'static> {
    let background = if selected {
        theme.selected
    } else {
        theme.panel
    };
    Line::from(vec![
        Span::styled(
            if selected { " › " } else { "   " },
            Style::default().fg(theme.accent).bg(background),
        ),
        Span::styled(
            command.label(),
            Style::default()
                .fg(theme.text)
                .bg(background)
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
    ])
}
