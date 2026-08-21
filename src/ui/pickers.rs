#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::too_many_lines,
    reason = "the picker renders headings and worktree rows in one pass"
)]
pub(super) fn draw_projects(
    frame: &mut Frame<'_>,
    groups: &[ProjectGroup],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    theme: &Theme,
) {
    let height = frame.area().height.saturating_sub(6).min(28);
    let area = centered_rect(
        frame.area().width.saturating_sub(10).min(88),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Recent projects ", theme);
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
            Paragraph::new("Loading projects…").style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let visible = App::filtered_project_rows(groups, &query.value);
        let mut lines = Vec::new();
        let mut selectable = 0_usize;
        let mut selected_line = 0_usize;
        for (group_index, group) in groups.iter().enumerate() {
            let trees: Vec<usize> = visible
                .iter()
                .filter_map(|(visible_group, tree_index)| {
                    (*visible_group == group_index).then_some(*tree_index)
                })
                .collect();
            if trees.is_empty() {
                continue;
            }
            lines.push(Line::from(Span::styled(
                format!(" {}", group.name),
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::BOLD)
                    .bg(theme.panel),
            )));
            for tree_index in trees {
                let Some(tree) = group.worktrees.get(tree_index) else {
                    continue;
                };
                let active = selectable == selected;
                if active {
                    selected_line = lines.len();
                }
                selectable = selectable.saturating_add(1);
                let background = if active { theme.selected } else { theme.panel };
                let mut flag = String::new();
                if tree.current {
                    flag.push_str("this session");
                } else if tree.locked.is_some() {
                    flag.push_str("locked");
                } else if tree.prunable.is_some() {
                    flag.push_str("prunable");
                }
                lines.push(Line::from(vec![
                    Span::styled(
                        if tree.current { " ● " } else { "   " },
                        Style::default()
                            .fg(if tree.current {
                                theme.success
                            } else {
                                theme.muted
                            })
                            .bg(background),
                    ),
                    Span::styled(
                        format!("- {}", tree.branch_label()),
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
                            "  {}",
                            truncate_middle(
                                &tree.path.display().to_string(),
                                list_area.width.saturating_sub(36) as usize
                            )
                        ),
                        Style::default().fg(theme.muted).bg(background),
                    ),
                    Span::styled(
                        if flag.is_empty() {
                            String::new()
                        } else {
                            format!("  {flag}")
                        },
                        Style::default().fg(theme.muted).bg(background),
                    ),
                ]));
            }
        }
        let offset = selected_line.saturating_sub(list_area.height.saturating_sub(1) as usize);
        let visible_lines: Vec<Line<'_>> = lines
            .into_iter()
            .skip(offset)
            .take(list_area.height as usize)
            .collect();
        if visible_lines.is_empty() {
            frame.render_widget(
                Paragraph::new("No projects match this filter")
                    .style(Style::default().fg(theme.muted)),
                list_area,
            );
        } else {
            frame.render_widget(Paragraph::new(visible_lines), list_area);
        }
    }
    draw_modal_hint(
        frame,
        area,
        "Enter open   Delete forget project   Esc close",
        theme,
    );
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
