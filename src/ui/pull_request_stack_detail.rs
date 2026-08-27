#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_stack_member_detail(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    stack_hits: &mut Vec<StackInspectorHitArea>,
    link_hits: &mut Vec<LinkHit>,
) -> (
    Option<Rect>,
    Vec<ContentFileHit>,
    Vec<ContentStepHit>,
    Vec<ContentReviewHit>,
) {
    let focused = app.focus == Focus::Content && app.modal.is_none();
    let block = panel_block(stack_member_panel_title(app), focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return (None, Vec::new(), Vec::new(), Vec::new());
    }
    let header_height = inner.height.min(3);
    let actions_height = u16::from(inner.height >= 5);
    let header = Rect::new(inner.x, inner.y, inner.width, header_height);
    let body = Rect::new(
        inner.x,
        inner.y.saturating_add(header_height),
        inner.width,
        inner
            .height
            .saturating_sub(header_height)
            .saturating_sub(actions_height),
    );
    draw_stack_member_identity(frame, header, app, theme, stack_hits, link_hits);
    if actions_height > 0 {
        draw_review_actions(
            frame,
            Rect::new(
                inner.x,
                inner.bottom().saturating_sub(actions_height),
                inner.width,
                actions_height,
            ),
            app,
            theme,
            stack_hits,
        );
    }
    if body.height == 0 {
        return (None, Vec::new(), Vec::new(), Vec::new());
    }
    if app.stack_inspector.section == StackMemberSection::Files {
        return draw_content(frame, body, app, theme, link_hits);
    }
    let width = usize::from(body.width.saturating_sub(1).max(1));
    let key = (
        app.stack_inspector.section,
        width,
        app.stack_inspector.content_generation,
        app.relative_time_generation,
    );
    if app.stack_inspector_content_rows_key != Some(key) {
        let (rows, links) = pull_request_stack_rows::stack_member_rows(app, width, theme);
        app.stack_inspector_content_width = rows
            .iter()
            .filter(|row| row.wide)
            .map(|row| row.line.width())
            .max()
            .unwrap_or_default();
        app.stack_inspector_content_rows = rows;
        app.stack_inspector_content_links = links;
        app.stack_inspector_content_rows_key = Some(key);
    }
    let rows = &app.stack_inspector_content_rows;
    let overflow = app.stack_inspector_content_width.saturating_sub(width);
    app.horizontal_scroll = app.horizontal_scroll.min(overflow);
    let max_scroll = rows.len().saturating_sub(usize::from(body.height));
    app.content_scroll = app.content_scroll.min(max_scroll);
    app.content_at_bottom = app.content_scroll == max_scroll;
    for (offset, row) in rows
        .iter()
        .skip(app.content_scroll)
        .take(usize::from(body.height))
        .enumerate()
    {
        let source_row = app.content_scroll.saturating_add(offset);
        let row_area = Rect::new(body.x, body.y.saturating_add(cells(offset)), body.width, 1);
        frame.render_widget(
            Paragraph::new(if row.wide {
                shift_line(&row.line, app.horizontal_scroll, width)
            } else {
                row.line.clone()
            })
            .style(Style::default().bg(theme.panel)),
            row_area,
        );
        for link in app
            .stack_inspector_content_links
            .iter()
            .filter(|link| link.row == source_row)
        {
            Link::new(link.target.clone()).register(
                horizontally_scrolled_link_area(
                    row_area,
                    if row.wide { app.horizontal_scroll } else { 0 },
                    link.start,
                    link.width,
                ),
                link_hits,
            );
        }
    }
    draw_scrollbar(frame, body, app.content_scroll, rows.len(), theme);
    (None, Vec::new(), Vec::new(), Vec::new())
}

fn stack_member_panel_title(app: &App) -> String {
    let position = app.pull_request_stack_cursor.unwrap_or_default();
    let size = app
        .pull_request_stack
        .as_ref()
        .map_or(0, |stack| stack.size);
    let cache = match app.stack_inspector.section {
        StackMemberSection::Files => "",
        StackMemberSection::Summary if app.stack_inspector.selected_from_cache => " · cached",
        StackMemberSection::Conversation if app.stack_inspector.conversation.from_cache => {
            " · cached"
        }
        StackMemberSection::Checks if app.stack_inspector.selected_checks().from_cache => {
            " · cached"
        }
        StackMemberSection::Commits if app.stack_inspector.commits.from_cache => " · cached",
        StackMemberSection::Summary
        | StackMemberSection::Conversation
        | StackMemberSection::Checks
        | StackMemberSection::Commits => "",
    };
    format!(" Member {position}/{size}{cache} ")
}

fn draw_stack_member_identity(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
    stack_hits: &mut Vec<StackInspectorHitArea>,
    link_hits: &mut Vec<LinkHit>,
) {
    let pull_request = app
        .stack_inspector
        .selected_pull_request
        .as_ref()
        .or(app.stack_inspector.selected_locator.as_ref());
    if let Some(pull_request) = pull_request {
        let state = if pull_request.is_draft {
            "DRAFT"
        } else {
            pull_request.state.as_str()
        };
        let prefix = format!(" #{} ", pull_request.number);
        let suffix = format!("  {state} ");
        let title = truncate_end(
            &pull_request.title,
            usize::from(area.width)
                .saturating_sub(prefix.width())
                .saturating_sub(suffix.width()),
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(prefix.clone(), Link::style(theme)),
                Span::styled(title.clone(), Link::style(theme)),
                Span::styled(
                    suffix,
                    Style::default()
                        .fg(stack_member_state_color(state, theme))
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
            .style(Style::default().bg(theme.panel_alt)),
            Rect::new(area.x, area.y, area.width, 1),
        );
        if !pull_request.url.is_empty() {
            Link::new(OpenTarget::Browser(pull_request.url.clone())).register(
                clipped_link_area(
                    area.x,
                    area.y,
                    prefix.width().saturating_add(title.width()),
                    area,
                ),
                link_hits,
            );
        }
        if area.height >= 2 {
            let metadata = format!(
                " @{} · {} -> {} · updated {}",
                pull_request.author,
                pull_request.base_ref,
                pull_request.head_ref,
                format_relative_timestamp(&pull_request.updated_at),
            );
            frame.render_widget(
                Paragraph::new(truncate_end(&metadata, usize::from(area.width)))
                    .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
                Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
            );
        }
    } else {
        frame.render_widget(
            Paragraph::new(" Loading stack member metadata…")
                .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
            Rect::new(area.x, area.y, area.width, area.height.min(2)),
        );
    }
    if area.height >= 3 {
        draw_stack_member_section_tabs(
            frame,
            Rect::new(area.x, area.y.saturating_add(2), area.width, 1),
            app,
            theme,
            stack_hits,
        );
    }
}

fn draw_stack_member_section_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
    hits: &mut Vec<StackInspectorHitArea>,
) {
    let sections = [
        StackMemberSection::Files,
        StackMemberSection::Summary,
        StackMemberSection::Conversation,
        StackMemberSection::Checks,
        StackMemberSection::Commits,
    ];
    let areas = Layout::horizontal([
        Constraint::Ratio(1, 5),
        Constraint::Ratio(1, 5),
        Constraint::Ratio(1, 5),
        Constraint::Ratio(1, 5),
        Constraint::Ratio(1, 5),
    ])
    .split(area);
    for (index, section) in sections.into_iter().enumerate() {
        let Some(tab) = areas.get(index).copied() else {
            continue;
        };
        let active = section == app.stack_inspector.section;
        frame.render_widget(
            Paragraph::new(stack_member_section_label(section, app))
                .alignment(Alignment::Center)
                .style(
                    Style::default()
                        .fg(if active { theme.text } else { theme.muted })
                        .bg(if active {
                            theme.accent_soft
                        } else {
                            theme.panel
                        })
                        .add_modifier(if active {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
            tab,
        );
        hits.push(StackInspectorHitArea {
            area: tab,
            target: StackInspectorHit::Section(section),
        });
    }
}

fn stack_member_section_label(section: StackMemberSection, app: &App) -> String {
    match section {
        StackMemberSection::Files => format!(
            "1 Files {}",
            app.stack_inspector
                .selected_pull_request
                .as_ref()
                .or(app.stack_inspector.selected_locator.as_ref())
                .map_or(0, |pull_request| pull_request.changed_files)
        ),
        StackMemberSection::Summary => "2 Summary".to_owned(),
        StackMemberSection::Conversation => format!(
            "3 Conversation {}",
            app.stack_inspector.conversation.comment_count()
        ),
        StackMemberSection::Checks => {
            format!(
                "4 Checks {}",
                app.stack_inspector.selected_checks().checks.len()
            )
        }
        StackMemberSection::Commits => {
            format!("5 Commits {}", app.stack_inspector.commits.total_commits)
        }
    }
}

fn draw_review_actions(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
    hits: &mut Vec<StackInspectorHitArea>,
) {
    let [previous, review, next] = Layout::horizontal([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(2, 4),
        Constraint::Ratio(1, 4),
    ])
    .areas(area);
    let position = app.pull_request_stack_cursor.unwrap_or_default();
    let size = app
        .pull_request_stack
        .as_ref()
        .map_or(0, |stack| stack.size);
    let review_enabled = app
        .selected_pull_request()
        .is_some_and(|pull_request| pull_request.state == "OPEN");
    let actions = [
        (
            previous,
            " [p Previous] ",
            StackInspectorHit::Previous,
            position > 1,
        ),
        (
            review,
            " [r Submit review] ",
            StackInspectorHit::Review,
            review_enabled,
        ),
        (next, " [n Next] ", StackInspectorHit::Next, position < size),
    ];
    for (target, label, hit, enabled) in actions {
        frame.render_widget(
            Paragraph::new(label).alignment(Alignment::Center).style(
                Style::default()
                    .fg(if enabled { theme.accent } else { theme.muted })
                    .bg(if hit == StackInspectorHit::Review {
                        theme.accent_soft
                    } else {
                        theme.panel_alt
                    })
                    .add_modifier(if enabled {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            target,
        );
        if enabled {
            hits.push(StackInspectorHitArea {
                area: target,
                target: hit,
            });
        }
    }
}

const fn stack_member_state_color(state: &str, theme: &Theme) -> Color {
    match state.as_bytes() {
        b"OPEN" => theme.success,
        b"MERGED" => theme.accent,
        b"CLOSED" => theme.removed,
        _ => theme.modified,
    }
}
