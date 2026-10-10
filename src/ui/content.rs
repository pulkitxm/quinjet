#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
pub(super) fn draw_content(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> (
    Option<Rect>,
    Vec<ContentFileHit>,
    Vec<ContentStepHit>,
    Vec<ContentReviewHit>,
) {
    if app.view == View::PullRequests && app.pull_request_section == PullRequestSection::Overview {
        let step_hits = draw_pull_request_overview(frame, area, app, theme, link_hits);
        return (None, Vec::new(), step_hits, Vec::new());
    }
    let file_action = if app.preview_files_collapsible() {
        if app.preview_files_all_collapsed() {
            "  [e Expand all]"
        } else {
            "  [e Collapse all]"
        }
    } else {
        ""
    };
    let review_action = if app.review_surface_active() {
        if app.pull_request_review_loading {
            "  [review loading]".to_owned()
        } else {
            format!(
                "  [c Comment · V Submit · {} pending]",
                app.pull_request_review.pending_comment_count()
            )
        }
    } else {
        String::new()
    };
    let loading = app.pull_request_progress.map_or_else(
        || {
            if app.document_loading
                && !(app.view == View::PullRequests && app.document.file_count() > 0)
            {
                "  · loading".to_owned()
            } else {
                String::new()
            }
        },
        |progress| format!("  · {}%", progress.percent()),
    );
    let wrap_label = if app.wrap_code {
        "  [Alt+z Unwrap]"
    } else {
        "  [Alt+z Wrap]"
    };
    let title_width = (area.width as usize)
        .saturating_sub(wrap_label.width())
        .saturating_sub(loading.width())
        .saturating_sub(file_action.width())
        .saturating_sub(review_action.width())
        .saturating_sub(jump_control_reserve())
        .saturating_sub(4);
    let title = format!(
        " {}{}{}{}{} ",
        truncate_middle(&app.document.title, title_width),
        loading,
        file_action,
        review_action,
        wrap_label,
    );
    let block = panel_block(
        title,
        app.focus == Focus::Content && app.modal.is_none(),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return (None, Vec::new(), Vec::new(), Vec::new());
    }

    let details_rows = if app.document.commit_details.is_some() {
        commit_details_row_count(inner.height)
    } else if app.document.pull_request_details.is_some() {
        pull_request_details_row_count(inner.height)
    } else {
        0
    };
    let side_by_side = app.diff_layout == DiffLayout::SideBySide && inner.width >= 72;
    let rows_key = (
        app.document_layout_generation,
        side_by_side,
        inner.width,
        app.diff_split_percent,
        app.wrap_code,
    );
    if app.diff_rows_key != Some(rows_key) {
        app.diff_restore_scroll |= app.wrap_code && app.diff_rows_key.is_some();
        if side_by_side {
            app.side_by_side_diff_rows = side_by_side_rows(&app.document, app);
            app.unified_diff_rows = Vec::new();
        } else {
            app.unified_diff_rows = unified_row_indices(&app.document, app);
            app.side_by_side_diff_rows = Vec::new();
        }
        diff_wrapping::wrap_diff_rows(app, side_by_side, inner.width.saturating_sub(1));
        app.diff_rows_key = Some(rows_key);
    }
    let diff_rows = if side_by_side {
        app.side_by_side_diff_rows.len()
    } else {
        app.unified_diff_rows.len()
    };
    if let Some(anchor) = app.content_file_anchor.take()
        && let Some(row) = anchored_diff_row(app, side_by_side, &anchor)
    {
        app.content_scroll = details_rows.saturating_add(row);
        app.pull_request_restore_scroll = false;
    }
    if std::mem::take(&mut app.pull_request_restore_scroll)
        && let Some((path, offset, scroll)) = &app.pull_request_scroll_anchor
        && *scroll == app.content_scroll
        && let Some(row) = anchored_diff_row(app, side_by_side, path)
    {
        app.content_scroll = details_rows.saturating_add(row).saturating_add(*offset);
    }
    diff_wrapping::restore_scroll(app, side_by_side, details_rows);
    let visual_length = details_rows + diff_rows;
    let max_scroll = visual_length.saturating_sub(inner.height as usize);
    app.content_scroll = app.content_scroll.min(max_scroll);
    app.content_at_bottom = app.content_scroll >= max_scroll;

    let mut diff_area = inner;
    let mut diff_scroll = app.content_scroll;
    if diff_scroll < details_rows {
        let visible_details = details_rows - diff_scroll;
        let details_height = cells(visible_details.min(inner.height as usize));
        let details_area = Rect::new(inner.x, inner.y, inner.width, details_height);
        if app.document.commit_details.is_some() {
            draw_commit_details_scrolled(
                frame,
                details_area,
                app,
                diff_scroll,
                details_rows,
                theme,
                link_hits,
            );
        } else if app.document.pull_request_details.is_some() {
            draw_pull_request_details_scrolled(
                frame,
                details_area,
                app,
                diff_scroll,
                details_rows,
                theme,
                link_hits,
            );
        }
        diff_area = Rect::new(
            inner.x,
            inner.y.saturating_add(details_height),
            inner.width,
            inner.height.saturating_sub(details_height),
        );
        diff_scroll = 0;
    } else {
        diff_scroll = diff_scroll.saturating_sub(details_rows);
    }

    pull_request_viewport::update_pull_request_diff_viewport(
        app,
        side_by_side,
        diff_scroll,
        inner.height as usize,
        details_rows,
    );
    let render_area = Rect::new(
        diff_area.x,
        diff_area.y,
        diff_area.width.saturating_sub(1),
        diff_area.height,
    );
    if app.content_scroll >= details_rows {
        diff_wrapping::remember_scroll(app, side_by_side, diff_scroll);
    } else {
        app.diff_scroll_anchor = None;
    }
    diff_scrolling::update_diff_scroll(app, render_area, side_by_side, diff_scroll);
    let mut content_review_hits = Vec::new();
    let (divider, content_file_hits) = if render_area.width < 2 || render_area.height == 0 {
        (None, Vec::new())
    } else if side_by_side {
        let (divider, hits) = draw_side_by_side_diff(
            frame,
            render_area,
            app,
            &app.side_by_side_diff_rows,
            diff_scroll,
            &mut content_review_hits,
            theme,
        );
        (Some(divider), hits)
    } else {
        let hits = draw_unified_diff(
            frame,
            render_area,
            app,
            &app.unified_diff_rows,
            diff_scroll,
            &mut content_review_hits,
            theme,
        );
        (None, hits)
    };
    app.geometry.content_scrollbar = crate::app::Scrollbar::new(
        inner,
        app.content_scroll,
        visual_length,
        usize::from(inner.height),
    );
    draw_scrollbar(frame, app.geometry.content_scrollbar, theme);
    (divider, content_file_hits, Vec::new(), content_review_hits)
}

pub(super) fn anchored_diff_row(app: &App, side_by_side: bool, anchor: &Path) -> Option<usize> {
    let matches_anchor = |index: &usize| {
        app.document
            .lines
            .get(*index)
            .and_then(file_header_path)
            .is_some_and(|path| Path::new(path) == anchor)
    };
    if side_by_side {
        app.side_by_side_diff_rows.iter().position(
            |row| matches!(row, SideBySideRow::FileHeader(index) if matches_anchor(index)),
        )
    } else {
        app.unified_diff_rows.iter().position(matches_anchor)
    }
}

const JUMP_TOP_LABEL: &str = " ↑ Top ";
pub(super) const JUMP_BOTTOM_LABEL: &str = " ↓ Bottom ";
pub(super) const JUMP_CONTROL_MARGIN: u16 = 3;

fn jump_control_reserve() -> usize {
    JUMP_TOP_LABEL
        .width()
        .saturating_add(JUMP_CONTROL_MARGIN as usize)
}

pub(super) fn draw_jump_controls(
    frame: &mut Frame<'_>,
    content: Rect,
    app: &App,
    theme: &Theme,
) -> Vec<ScmActionHit> {
    if app.modal.is_some() || content.width < 20 || content.height < 3 {
        return Vec::new();
    }
    let mut hits = Vec::new();
    if app.content_scroll > 0 {
        hits.push(draw_jump_control(
            frame,
            content,
            JUMP_TOP_LABEL,
            content.y,
            ScmAction::JumpToTop,
            theme,
        ));
    }
    if !app.content_at_bottom {
        hits.push(draw_jump_control(
            frame,
            content,
            JUMP_BOTTOM_LABEL,
            content.bottom().saturating_sub(1),
            ScmAction::JumpToBottom,
            theme,
        ));
    }
    hits
}

fn draw_jump_control(
    frame: &mut Frame<'_>,
    content: Rect,
    label: &'static str,
    y: u16,
    action: ScmAction,
    theme: &Theme,
) -> ScmActionHit {
    let width = cells(label.width());
    let area = Rect::new(
        content
            .right()
            .saturating_sub(width.saturating_add(JUMP_CONTROL_MARGIN))
            .max(content.x),
        y,
        width,
        1,
    );
    frame.render_widget(
        Paragraph::new(label).style(
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        ),
        area,
    );
    ScmActionHit { area, action }
}

#[expect(
    clippy::too_many_arguments,
    reason = "rendering and hit registration share the same scrolled coordinate space"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the details card reads better as one top-to-bottom pass"
)]
pub(super) fn draw_commit_details_scrolled(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    scroll: usize,
    total_rows: usize,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) {
    let Some(details) = app.document.commit_details.as_ref() else {
        return;
    };
    let document = &app.document;
    let load_progress = app.local_diff_load_progress();
    let (additions, deletions) = app
        .local_diff_line_counts()
        .unwrap_or_else(|| (document.addition_count(), document.deletion_count()));
    let block = Block::default()
        .title(" Commit details ")
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.panel_alt).fg(theme.text));
    let full_area = Rect::new(0, 0, area.width, cells(total_rows));
    let mut buffer = Buffer::empty(full_area);
    let inner = block.inner(full_area);
    block.render(full_area, &mut buffer);
    let file_count = document.file_count();
    let subject = pull_request_reference(&details.subject)
        .and_then(|(start, end, number)| {
            Some((
                details.subject.get(..start)?,
                details.subject.get(start..end)?,
                details.subject.get(end..)?,
                app.pull_request_open_target(number)?,
            ))
        })
        .map_or_else(
            || {
                Line::from(Span::styled(
                    details.subject.as_str(),
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ))
            },
            |(prefix, reference, suffix, target)| {
                Line::from(vec![
                    Span::styled(
                        prefix,
                        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                    ),
                    link_span(
                        reference.to_owned(),
                        Some(target),
                        scrolled_detail_link_area(
                            area,
                            scroll,
                            inner.y,
                            inner.x.saturating_add(cells(prefix.width())),
                            reference.width(),
                        ),
                        theme,
                        link_hits,
                    ),
                    Span::styled(
                        suffix,
                        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                    ),
                ])
            },
        );
    let lines = vec![
        subject,
        detail_line(
            "Author",
            format!(
                "{} <{}>  ·  {}",
                details.author,
                details.author_email,
                format_relative_timestamp(&details.authored_at)
            ),
            theme,
        ),
        detail_line(
            "Committer",
            format!(
                "{} <{}>  ·  {}",
                details.committer,
                details.committer_email,
                format_relative_timestamp(&details.committed_at)
            ),
            theme,
        ),
        Line::from(vec![
            Span::styled(
                format!("{:<DETAIL_LABEL_WIDTH$}", "Commit"),
                Style::default().fg(theme.muted),
            ),
            link_span(
                details.id.clone(),
                app.commit_open_target(&details.id),
                scrolled_detail_link_area(
                    area,
                    scroll,
                    inner.y.saturating_add(3),
                    inner.x.saturating_add(cells(DETAIL_LABEL_WIDTH)),
                    details.id.width(),
                ),
                theme,
                link_hits,
            ),
        ]),
        Line::from(vec![
            Span::styled("Changes    ", Style::default().fg(theme.muted)),
            Span::styled(
                format!(
                    "{} file{} changed{}  ",
                    file_count,
                    if file_count == 1 { "" } else { "s" },
                    load_progress.map_or_else(String::new, |(loaded, total)| {
                        format!("  ·  {loaded}/{total} diffs loaded")
                    }),
                ),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("+{additions}"),
                Style::default()
                    .fg(theme.added)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("-{deletions}"),
                Style::default()
                    .fg(theme.removed)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];
    Paragraph::new(lines).render(inner, &mut buffer);

    for destination_row in 0..area.height {
        let source_row = cells(scroll) + destination_row;
        if source_row >= full_area.height {
            break;
        }
        for column in 0..area.width {
            let source = buffer[(column, source_row)].clone();
            if let Some(destination) = frame
                .buffer_mut()
                .cell_mut((area.x + column, area.y + destination_row))
            {
                *destination = source;
            }
        }
    }
}
