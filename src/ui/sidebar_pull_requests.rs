#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
pub(super) fn draw_pull_requests_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> (Vec<SidebarHitArea>, Vec<ScmActionHit>) {
    let warning = if app.pull_request_warnings.is_empty() {
        String::new()
    } else {
        format!("  ⚠{}", app.pull_request_warnings.len())
    };
    let loading = app
        .pull_request_progress
        .map_or_else(String::new, |progress| {
            format!("  · {}%", progress.percent())
        });
    let cache = if app.pull_request_refreshing() {
        "  · refreshing"
    } else if app.pull_request_served_from_cache() {
        "  · cached"
    } else {
        ""
    };
    let title = app.selected_pull_request().map_or_else(
        || {
            if app.recent_pull_requests.is_empty() {
                format!(" Open Pull Request · on demand{loading}{warning} ")
            } else {
                format!(" Recent Pull Requests{loading}{warning} ")
            }
        },
        |pull_request| {
            let state = if pull_request.is_draft {
                "DRAFT"
            } else {
                pull_request.state.as_str()
            };
            format!(
                " Pull Request #{} · {state}{loading}{cache}{warning} ",
                pull_request.number
            )
        },
    );
    let block = panel_block(
        title,
        app.focus == Focus::Sidebar && app.modal.is_none(),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return (Vec::new(), Vec::new());
    }

    let has_cta = app.selected_pull_request().is_some() && app.pr_primary_action().is_some();
    let base_controls = if has_cta { 3_u16 } else { 2_u16 };
    let controls_height = inner.height.min(base_controls);
    let body_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(controls_height),
    );
    let mut hits = Vec::new();
    let mut action_hits = Vec::new();
    if app.pull_request.is_some() && body_area.height > 0 {
        let has_stack = app.pull_request_stack.is_some();
        let overview_width = if has_stack {
            body_area.width / 3
        } else {
            body_area.width.saturating_mul(3) / 5
        };
        let files_width = if has_stack {
            body_area.width / 3
        } else {
            body_area.width.saturating_sub(overview_width)
        };
        let overview_tab = Rect::new(body_area.x, body_area.y, overview_width, 1);
        let files_tab = Rect::new(overview_tab.right(), body_area.y, files_width, 1);
        let stack_tab = has_stack.then(|| {
            Rect::new(
                files_tab.right(),
                body_area.y,
                body_area
                    .width
                    .saturating_sub(overview_width)
                    .saturating_sub(files_width),
                1,
            )
        });
        let overview_label = "PR".to_owned();
        draw_pull_request_section_tab(
            frame,
            overview_tab,
            overview_label,
            app.pull_request_section == PullRequestSection::Overview,
            theme,
        );
        if let (Some(stack), Some(tab)) = (app.pull_request_stack.as_ref(), stack_tab) {
            draw_pull_request_section_tab(
                frame,
                tab,
                format!("Stack {}", stack.size),
                app.pull_request_section == PullRequestSection::Stack,
                theme,
            );
        }
        draw_pull_request_section_tab(
            frame,
            files_tab,
            format!("Files {}", app.pull_request_total_files),
            app.pull_request_section == PullRequestSection::Files,
            theme,
        );
        hits.push(SidebarHitArea {
            area: overview_tab,
            target: SidebarHit::PullRequestOverview,
        });
        hits.push(SidebarHitArea {
            area: files_tab,
            target: SidebarHit::PullRequestFiles,
        });
        if let Some(tab) = stack_tab {
            hits.push(SidebarHitArea {
                area: tab,
                target: SidebarHit::PullRequestStack,
            });
        }

        let list_area = Rect::new(
            body_area.x,
            body_area.y + 1,
            body_area.width,
            body_area.height.saturating_sub(1),
        );
        match app.pull_request_section {
            PullRequestSection::Files => {
                hits.extend(draw_pull_request_file_tree(frame, list_area, app, theme));
            }
            PullRequestSection::Overview => {
                hits.extend(draw_pull_request_check_list(frame, list_area, app, theme));
            }
            PullRequestSection::Stack => {
                hits.extend(pull_request_stack::draw_pull_request_stack(
                    frame, list_area, app, theme,
                ));
            }
        }
    } else if app.pull_request_loading {
        let skeleton_count = body_area.height.min(6);
        for offset in 0..skeleton_count {
            let y = body_area.y + offset;
            let width = body_area.width.saturating_sub(8 + (offset % 3) * 5);
            frame.render_widget(
                Paragraph::new(format!(
                    "   ◌ {}",
                    "─".repeat(width.saturating_sub(6) as usize)
                ))
                .style(Style::default().fg(theme.border).bg(theme.panel)),
                Rect::new(body_area.x, y, body_area.width, 1),
            );
        }
    } else if body_area.height > 0 {
        hits.extend(draw_recent_pull_requests(
            frame, body_area, app, theme, link_hits,
        ));
    }

    let controls_y = body_area.bottom();
    let repository_name = app
        .pull_request_repository
        .as_ref()
        .or(app.local_github_repository.as_ref())
        .map_or_else(
            || "auto-detect from remotes".to_owned(),
            GitHubRepository::display_name,
        );
    if controls_height >= 1 {
        let repository_area = Rect::new(inner.x, controls_y, inner.width, 1);
        let prefix = " repo ";
        let visible_name = truncate_middle(
            &repository_name,
            usize::from(inner.width).saturating_sub(prefix.width()),
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(prefix),
                link_span(
                    visible_name.clone(),
                    app.pull_request_repository_open_target(),
                    clipped_link_area(
                        repository_area.x.saturating_add(cells(prefix.width())),
                        repository_area.y,
                        visible_name.width(),
                        repository_area,
                    ),
                    theme,
                    link_hits,
                ),
            ]))
            .style(Style::default().fg(theme.text).bg(theme.panel_alt)),
            repository_area,
        );
        hits.push(SidebarHitArea {
            area: repository_area,
            target: SidebarHit::PullRequestChooseRepository,
        });
    }
    if controls_height >= 2 {
        let lookup_area = Rect::new(inner.x, controls_y + 1, inner.width, 1);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" PR # ", Style::default().fg(theme.accent)),
                Span::styled(
                    app.pull_request_lookup.value.as_str(),
                    Style::default().fg(theme.text),
                ),
            ]))
            .style(Style::default().bg(if app.pull_request_lookup_active {
                theme.selected
            } else {
                theme.panel_alt
            })),
            lookup_area,
        );
        if app.pull_request_lookup_active {
            set_text_cursor(
                frame,
                Rect::new(
                    lookup_area.x + 6,
                    lookup_area.y,
                    lookup_area.width.saturating_sub(6),
                    1,
                ),
                &app.pull_request_lookup,
                false,
            );
        }
        hits.push(SidebarHitArea {
            area: lookup_area,
            target: SidebarHit::PullRequestLookup,
        });
    }
    if has_cta && controls_height >= 3 {
        let row = Rect::new(inner.x, controls_y + 2, inner.width, 1);
        draw_pull_request_cta(frame, row, app, theme, &mut action_hits);
    }
    (hits, action_hits)
}

pub(super) fn draw_pull_request_cta(
    frame: &mut Frame<'_>,
    row: Rect,
    app: &App,
    theme: &Theme,
    action_hits: &mut Vec<ScmActionHit>,
) {
    let Some(primary) = app.pr_primary_action() else {
        return;
    };
    let menu_items = app.pr_menu_items();
    let show_menu = !menu_items.is_empty();
    let arrow_width = if show_menu { 3.min(row.width) } else { 0 };
    let label_width = row.width.saturating_sub(arrow_width);
    let label_area = Rect::new(row.x, row.y, label_width, 1);
    let arrow_area = Rect::new(row.x.saturating_add(label_width), row.y, arrow_width, 1);
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(theme.panel_alt)),
        row,
    );
    frame.render_widget(
        Paragraph::new(primary.label())
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.panel_alt)
                    .add_modifier(Modifier::BOLD),
            ),
        label_area,
    );
    action_hits.push(ScmActionHit {
        area: label_area,
        action: ScmAction::PrPrimary,
    });
    if show_menu {
        frame.render_widget(
            Paragraph::new("▶")
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
            arrow_area,
        );
        action_hits.push(ScmActionHit {
            area: arrow_area,
            action: ScmAction::PrToggleMenu,
        });
        if app.pr_menu_open {
            draw_pr_menu(
                frame,
                row,
                &menu_items,
                app.pr_menu_selected,
                theme,
                action_hits,
            );
        }
    }
}

pub(super) fn draw_pr_menu(
    frame: &mut Frame<'_>,
    anchor: Rect,
    items: &[PrMenuItem],
    selected: usize,
    theme: &Theme,
    action_hits: &mut Vec<ScmActionHit>,
) {
    let width = items
        .iter()
        .map(|item| item.label().width())
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
    for (index, item) in items.iter().enumerate() {
        let active = index == selected;
        let row = Rect::new(
            inner.x,
            inner.y.saturating_add(u16::try_from(index).unwrap_or(0)),
            inner.width,
            1,
        );
        frame.render_widget(
            Paragraph::new(format!(" {} ", item.label())).style(if active {
                Style::default().fg(theme.text).bg(theme.selected)
            } else {
                Style::default().fg(theme.text).bg(theme.panel)
            }),
            row,
        );
        action_hits.push(ScmActionHit {
            area: row,
            action: ScmAction::PrMenu(*item),
        });
    }
}

pub(super) fn draw_recent_pull_requests(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> Vec<SidebarHitArea> {
    if app.recent_pull_requests.is_empty() {
        frame.render_widget(
            Paragraph::new(" No recently opened pull requests")
                .style(Style::default().fg(theme.muted).bg(theme.panel)),
            area,
        );
        return Vec::new();
    }

    let heading = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(" Recently opened").style(Style::default().fg(theme.muted).bg(theme.panel)),
        heading,
    );
    let capacity = usize::from(area.height.saturating_sub(1)).div_euclid(2);
    if capacity == 0 {
        return Vec::new();
    }
    let start = app
        .recent_pull_request_cursor
        .saturating_sub(capacity.saturating_sub(1));
    app.recent_pull_requests
        .iter()
        .enumerate()
        .skip(start)
        .take(capacity)
        .map(|(index, pull_request)| {
            let selected = index == app.recent_pull_request_cursor;
            let row = Rect::new(
                area.x,
                area.y + 1 + cells(index.saturating_sub(start).saturating_mul(2)),
                area.width,
                2,
            );
            let number = format!("#{}", pull_request.number);
            let title = truncate_middle(
                &pull_request.title,
                usize::from(area.width).saturating_sub(4),
            );
            let row_style = Style::default().bg(if selected {
                theme.selected
            } else {
                theme.panel
            });
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        if selected { " › " } else { "   " },
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(title, Style::default().fg(theme.text)),
                ]))
                .style(row_style),
                Rect::new(row.x, row.y, row.width, 1),
            );
            let metadata_x = row.x.saturating_add(3);
            let metadata_width = usize::from(row.width.saturating_sub(3));
            let age = truncate_end(
                &format_relative_timestamp(&pull_request.updated_at),
                metadata_width.saturating_sub(number.width() + 1),
            );
            let gap = metadata_width
                .saturating_sub(number.width())
                .saturating_sub(age.width());
            let number_area =
                clipped_link_area(metadata_x, row.y.saturating_add(1), number.width(), row);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    link_span(
                        number,
                        Some(OpenTarget::Browser(format!(
                            "{}/pull/{}",
                            pull_request.repository.url.trim_end_matches('/'),
                            pull_request.number
                        ))),
                        number_area,
                        theme,
                        link_hits,
                    ),
                    Span::raw(" ".repeat(gap)),
                    Span::styled(age, Style::default().fg(theme.muted)),
                ]))
                .style(row_style),
                Rect::new(
                    metadata_x,
                    row.y.saturating_add(1),
                    cells(metadata_width),
                    1,
                ),
            );
            SidebarHitArea {
                area: row,
                target: SidebarHit::RecentPullRequest(index),
            }
        })
        .collect()
}
