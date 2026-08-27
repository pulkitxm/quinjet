#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::too_many_lines,
    reason = "the root draw pass keeps geometry assignment in one coordinate space"
)]
pub(crate) fn draw(frame: &mut Frame<'_>, app: &mut App, theme: &Theme) {
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.background)),
        frame.area(),
    );

    if frame.area().width < 72 || frame.area().height < 18 {
        draw_too_small(frame, theme);
        return;
    }

    let repository_tabs_height = u16::from(app.repository_tabs.len() > 1).saturating_mul(2);
    let [repository_tabs, tabs, main, footer] = Layout::vertical([
        Constraint::Length(repository_tabs_height),
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(2),
    ])
    .areas(frame.area());
    let maximum_sidebar = main.width.saturating_sub(32).max(22);
    app.sidebar_width = app.sidebar_width.clamp(22, maximum_sidebar);

    let (repository_tab_hits, repository_tab_open, repository_tab_previous, repository_tab_next) =
        if repository_tabs_height == 0 {
            (
                Vec::new(),
                Rect::default(),
                Rect::default(),
                Rect::default(),
            )
        } else {
            draw_repository_tabs(frame, repository_tabs, app, theme)
        };
    let (changes_tab, history_tab, pull_requests_tab, mut link_hits, projects_hit) =
        draw_tabs(frame, tabs, app, theme);
    let mut project_hits: Vec<Rect> = projects_hit.into_iter().collect();
    let stack_workspace = app.view == View::PullRequests && app.pull_request_stack.is_some();
    let (workspace, mut scm_action_hits) = if stack_workspace {
        (
            pull_request_stack::draw_pull_request_stack_workspace(
                frame,
                main,
                app,
                theme,
                &mut link_hits,
            ),
            Vec::new(),
        )
    } else {
        let [sidebar, sidebar_divider, content] = if app.sidebar_hidden {
            [Rect::default(), Rect::default(), main]
        } else {
            Layout::horizontal([
                Constraint::Length(app.sidebar_width),
                Constraint::Length(1),
                Constraint::Min(31),
            ])
            .areas(main)
        };
        let (sidebar_hits, action_hits) = if app.sidebar_hidden {
            (Vec::new(), Vec::new())
        } else {
            draw_sidebar(frame, sidebar, app, theme, &mut link_hits)
        };
        if !app.sidebar_hidden {
            draw_main_divider(frame, sidebar_divider, app.resize_target.is_some(), theme);
        }
        let (diff_divider, content_file_hits, content_step_hits, content_review_hits) =
            draw_content(frame, content, app, theme, &mut link_hits);
        (
            pull_request_stack::StackWorkspaceGeometry {
                sidebar,
                sidebar_divider,
                content,
                diff_divider,
                sidebar_hits,
                stack_inspector_hits: Vec::new(),
                content_file_hits,
                content_step_hits,
                content_review_hits,
            },
            action_hits,
        )
    };
    scm_action_hits.extend(draw_jump_controls(frame, workspace.content, app, theme));
    draw_footer(frame, footer, app, theme, &mut link_hits, &mut project_hits);

    app.geometry = UiGeometry {
        repository_tab_hits,
        repository_tab_open,
        repository_tab_previous,
        repository_tab_next,
        repository_tab_menu_hits: Vec::new(),
        changes_tab,
        history_tab,
        pull_requests_tab,
        main,
        sidebar: workspace.sidebar,
        sidebar_divider: workspace.sidebar_divider,
        content: workspace.content,
        diff_divider: workspace.diff_divider,
        sidebar_hits: workspace.sidebar_hits,
        stack_inspector_hits: workspace.stack_inspector_hits,
        scm_action_hits,
        modal_action_hits: Vec::new(),
        modal_list_hits: Vec::new(),
        modal_list_len: 0,
        modal_list_max_scroll: 0,
        content_file_hits: workspace.content_file_hits,
        content_step_hits: workspace.content_step_hits,
        content_review_hits: workspace.content_review_hits,
        link_hits,
        help_hits: Vec::new(),
        project_hits,
        project_collapse_hits: Vec::new(),
    };

    let frame_area = frame.area();
    app.rendered_cells = snapshot_cells(frame.buffer_mut(), frame_area);
    if let Some(selection) = app.text_selection {
        draw_text_selection(frame, selection, theme);
    }
    if app.modal.is_none() {
        if !app.mouse_capture || app.link_hover.is_some() {
            draw_terminal_links(
                frame,
                &app.geometry.link_hits,
                app.mouse_capture.then_some(app.link_hover).flatten(),
            );
        }
        draw_link_hover(frame, &app.geometry.link_hits, app.link_hover);
    }

    if app.modal.is_some() {
        draw_modal(frame, app, theme);
    }
    if let Some(toast) = app.toast.as_ref() {
        draw_toast(frame, toast.message.as_str(), toast.level, theme);
    }
    draw_repository_tab_menu(frame, app, theme);
}

pub(super) fn snapshot_cells(buffer: &Buffer, area: Rect) -> Vec<Vec<char>> {
    (area.y..area.bottom())
        .map(|row| {
            (area.x..area.right())
                .map(|column| buffer[(column, row)].symbol().chars().next().unwrap_or(' '))
                .collect()
        })
        .collect()
}

pub(super) fn draw_text_selection(
    frame: &mut Frame<'_>,
    selection: crate::app::TextSelection,
    theme: &Theme,
) {
    let ((start_x, start_y), (end_x, end_y)) = selection.ordered_endpoints();
    for row in start_y..=end_y {
        let first = if row == start_y {
            start_x
        } else {
            selection.pane.x
        };
        let last = if row == end_y {
            end_x
        } else {
            selection.pane.right().saturating_sub(1)
        };
        for column in first..=last {
            if let Some(cell) = frame.buffer_mut().cell_mut((column, row)) {
                cell.fg = theme.text;
                cell.bg = theme.selected;
            }
        }
    }
}

pub(super) fn draw_terminal_links(
    frame: &mut Frame<'_>,
    hits: &[LinkHit],
    hover: Option<(u16, u16)>,
) {
    for hit in hits {
        if hover.is_some_and(|point| !hit.area.contains(point.into())) {
            continue;
        }
        let OpenTarget::Browser(url) = &hit.target;
        if url.chars().any(char::is_control) {
            continue;
        }
        for row in hit.area.y..hit.area.bottom() {
            for column in hit.area.x..hit.area.right() {
                let Some(cell) = frame.buffer_mut().cell_mut((column, row)) else {
                    continue;
                };
                let symbol = cell.symbol().to_owned();
                cell.set_symbol(&format!("\x1b]8;;{url}\x1b\\{symbol}\x1b]8;;\x1b\\"))
                    .diff_option = CellDiffOption::ForcedWidth(NonZeroU16::MIN);
            }
        }
    }
}

pub(super) fn draw_link_hover(frame: &mut Frame<'_>, hits: &[LinkHit], hover: Option<(u16, u16)>) {
    let Some(hover) = hover else {
        return;
    };
    let Some(hit) = hits.iter().find(|hit| hit.area.contains(hover.into())) else {
        return;
    };
    for row in hit.area.y..hit.area.bottom() {
        for column in hit.area.x..hit.area.right() {
            if let Some(cell) = frame.buffer_mut().cell_mut((column, row)) {
                cell.modifier.insert(Modifier::UNDERLINED);
            }
        }
    }
}

pub(super) fn draw_main_divider(frame: &mut Frame<'_>, area: Rect, dragging: bool, theme: &Theme) {
    let color = if dragging {
        theme.border_focus
    } else {
        theme.border
    };
    for row in area.y..area.bottom() {
        frame.render_widget(
            Paragraph::new("│").style(Style::default().fg(color).bg(theme.background)),
            Rect::new(area.x, row, 1, 1),
        );
    }
}

pub(super) fn draw_too_small(frame: &mut Frame<'_>, theme: &Theme) {
    let text = Text::from(vec![
        Line::from(Span::styled(
            "Quinjet",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Terminal too small",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "Resize to at least 72 × 18",
            Style::default().fg(theme.muted),
        )),
    ]);
    frame.render_widget(
        Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border)),
        ),
        centered_rect(50, 8, frame.area()),
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "the header renders its linked regions in one coordinate space"
)]
pub(super) fn draw_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
) -> (Rect, Rect, Rect, Vec<LinkHit>, Option<Rect>) {
    let mut link_hits = Vec::new();
    let branch = if app.status.branch.head.is_empty() {
        "detecting branch…".to_owned()
    } else {
        let mut text = format!(" {}", app.status.branch.head);
        if app.status.branch.ahead > 0 {
            text.push_str("  ↑");
            text.push_str(&app.status.branch.ahead.to_string());
        }
        if app.status.branch.behind > 0 {
            text.push_str("  ↓");
            text.push_str(&app.status.branch.behind.to_string());
        }
        text
    };
    let [
        changes_tab,
        history_tab,
        pull_requests_tab,
        title_area,
        branch_area,
    ] = Layout::horizontal([
        Constraint::Length(13),
        Constraint::Length(13),
        Constraint::Length(17),
        Constraint::Min(8),
        Constraint::Length(cells((branch.width() + 3).min(area.width as usize))),
    ])
    .areas(area);

    draw_tab(
        frame,
        changes_tab,
        "  Changes  ",
        app.view == View::Changes,
        theme,
    );
    draw_tab(
        frame,
        history_tab,
        "  History  ",
        app.view == View::History,
        theme,
    );
    draw_tab(
        frame,
        pull_requests_tab,
        " Pull Requests ",
        app.view == View::PullRequests,
        theme,
    );
    let prefix = " QUINJET ";
    let repository_x = title_area.x.saturating_add(cells(prefix.width()));
    let repository_area = clipped_link_area(
        repository_x,
        title_area.y.saturating_add(1),
        app.repository_name.width(),
        title_area,
    );
    let projects_hit = Some(repository_area).filter(|area| area.width > 0 && area.height > 0);
    let title = vec![
        Span::styled(
            prefix,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            app.repository_name.clone(),
            Style::default()
                .fg(theme.text)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
    ];
    frame.render_widget(
        Paragraph::new(Line::from(title)).block(
            Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.panel)),
        ),
        title_area,
    );
    let branch_line = if app.status.branch.head.is_empty() {
        Line::from(branch)
    } else {
        let branch_start = branch_area
            .right()
            .saturating_sub(1)
            .saturating_sub(cells(branch.width()));
        let branch_name_x = branch_start.saturating_add(cells(" ".width()));
        let branch_target = if app.status.branch.detached {
            app.status
                .branch
                .oid
                .as_deref()
                .and_then(|oid| app.commit_open_target(oid))
        } else {
            app.branch_open_target(&app.status.branch.head)
        };
        let branch_name = app.status.branch.head.clone();
        let branch_suffix = branch
            .strip_prefix(&format!(" {branch_name}"))
            .unwrap_or_default()
            .to_owned();
        Line::from(vec![
            Span::raw(" "),
            link_span(
                branch_name.clone(),
                branch_target,
                clipped_link_area(
                    branch_name_x,
                    branch_area.y.saturating_add(1),
                    branch_name.width(),
                    branch_area,
                ),
                theme,
                &mut link_hits,
            ),
            Span::raw(branch_suffix),
        ])
    };
    frame.render_widget(
        Paragraph::new(branch_line)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.accent).bg(theme.panel))
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(Style::default().fg(theme.border)),
            ),
        branch_area,
    );
    (
        changes_tab,
        history_tab,
        pull_requests_tab,
        link_hits,
        projects_hit,
    )
}

pub(super) fn draw_tab(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    active: bool,
    theme: &Theme,
) {
    let style = if active {
        Style::default()
            .fg(theme.text)
            .bg(theme.accent_soft)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted).bg(theme.panel)
    };
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Center)
            .style(style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if active {
                        theme.border_focus
                    } else {
                        theme.border
                    })),
            ),
        area,
    );
}
