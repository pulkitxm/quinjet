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
    mode: ProjectOpenMode,
    ssh: Option<&SshContext>,
    theme: &Theme,
) {
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
    let machine_rows = u16::from(ssh.is_some());
    if let Some(context) = ssh {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" SSH  ", Style::default().fg(theme.muted)),
                Span::styled(
                    context.current.as_str(),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("   Tab switch machine", Style::default().fg(theme.muted)),
            ]))
            .style(Style::default().bg(theme.panel)),
            Rect::new(inner.x, inner.y + 2, inner.width, 1),
        );
    }
    let list_area = Rect::new(
        inner.x,
        inner.y + 2 + machine_rows.saturating_mul(2),
        inner.width,
        inner
            .height
            .saturating_sub(5 + machine_rows.saturating_mul(2)),
    );
    if loading {
        frame.render_widget(
            Paragraph::new("Loading projects…").style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let matching = App::matching_project_rows(groups, &query.value);
        let mut lines: Vec<(Line<'static>, Option<std::path::PathBuf>)> = Vec::new();
        let mut selectable = 0_usize;
        let mut selected_line = 0_usize;
        for (group_index, group) in groups.iter().enumerate() {
            let trees: Vec<usize> = matching
                .iter()
                .filter_map(|(matching_group, tree_index)| {
                    (*matching_group == group_index).then_some(*tree_index)
                })
                .collect();
            if trees.is_empty() {
                continue;
            }
            let expanded = !collapsed.contains(&group.common_dir) || !query.value.is_empty();
            lines.push((
                project_header_line(group, expanded, list_area.width as usize, theme),
                Some(group.common_dir.clone()),
            ));
            if !expanded {
                continue;
            }
            for tree_index in trees {
                let Some(tree) = group.worktrees.get(tree_index) else {
                    continue;
                };
                let active = selectable == selected;
                if active {
                    selected_line = lines.len();
                }
                selectable = selectable.saturating_add(1);
                lines.push((
                    project_worktree_line(tree, active, list_area.width as usize, theme),
                    None,
                ));
            }
        }
        let offset = selected_line.saturating_sub(list_area.height.saturating_sub(1) as usize);
        let visible_lines: Vec<_> = lines
            .into_iter()
            .skip(offset)
            .take(list_area.height as usize)
            .collect();
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
                |(line_index, (_, common_dir))| {
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
            frame.render_widget(
                Paragraph::new(
                    visible_lines
                        .into_iter()
                        .map(|(line, _)| line)
                        .collect::<Vec<_>>(),
                ),
                list_area,
            );
        }
    }
    let hint = match mode {
        ProjectOpenMode::Initial => "Enter open   Ctrl+E collapse all   Ctrl+O path   Esc quit",
        ProjectOpenMode::CurrentTab if ssh.is_some() => {
            "Enter switch tab   Ctrl+E collapse all   Tab machines   Delete forget project   Esc close"
        }
        ProjectOpenMode::NewTab if ssh.is_some() => {
            "Enter open in new tab   Ctrl+E collapse all   Tab machines   Delete forget project   Esc close"
        }
        ProjectOpenMode::CurrentTab => {
            "Enter switch tab   Ctrl+E collapse all   Delete forget project   Esc close"
        }
        ProjectOpenMode::NewTab => {
            "Enter open in new tab   Ctrl+E collapse all   Delete forget project   Esc close"
        }
    };
    draw_modal_hint(frame, area, hint, theme);
}

pub(super) fn draw_pull_request_repositories(
    frame: &mut Frame<'_>,
    items: &[GitHubRepository],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
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
    let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
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
    let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
    let lines = commands
        .iter()
        .skip(offset)
        .take(list_area.height as usize)
        .enumerate()
        .map(|(index, command)| palette_line(*command, offset + index == selected, theme))
        .collect::<Vec<_>>();
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

pub(super) fn draw_theme_picker(
    frame: &mut Frame<'_>,
    selected: usize,
    current: ThemeName,
    theme: &Theme,
) {
    let choices = ThemeName::ALL.map(|name| (name.label(), name == current));
    draw_choice_picker(frame, " Select Theme ", &choices, selected, theme);
}

pub(super) fn draw_appearance_picker(
    frame: &mut Frame<'_>,
    selected: usize,
    current: AppearanceChoice,
    theme: &Theme,
) {
    let choices = AppearanceChoice::ALL.map(|choice| (choice.label(), choice == current));
    draw_choice_picker(frame, " Select Appearance ", &choices, selected, theme);
}

pub(super) fn draw_choice_picker(
    frame: &mut Frame<'_>,
    title: &str,
    choices: &[(&'static str, bool)],
    selected: usize,
    theme: &Theme,
) {
    let height = (cells(choices.len()) + 4)
        .min(frame.area().height.saturating_sub(6))
        .max(7);
    let area = centered_rect(44, height, frame.area());
    frame.render_widget(Clear, area);
    let block = modal_block(title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let list_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(1),
    );
    let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
    let lines = choices
        .iter()
        .skip(offset)
        .take(list_area.height as usize)
        .enumerate()
        .map(|(index, (label, current))| {
            choice_line(label, *current, offset + index == selected, theme)
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), list_area);
    draw_modal_hint(frame, area, "Enter apply   Esc close", theme);
}

pub(super) fn choice_line(
    label: &'static str,
    current: bool,
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
            if current { "✓ " } else { "  " },
            Style::default().fg(theme.success).bg(background),
        ),
        Span::styled(
            label,
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
