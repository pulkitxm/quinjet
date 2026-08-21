use super::*;

pub(super) fn draw_full_width_diff_line(
    frame: &mut Frame<'_>,
    area: Rect,
    line: &DiffLine,
    _boxed: bool,
    horizontal_scroll: usize,
    theme: &Theme,
) {
    let content_area = area;
    let (marker, marker_style) = marker_for(line.kind, theme);
    let mut spans = vec![Span::styled(marker, marker_style)];
    spans.extend(highlight_spans(
        &line.spans,
        horizontal_scroll,
        content_area.width.saturating_sub(2) as usize,
        line.kind,
        None,
        theme,
    ));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(line_background(line.kind, theme)),
        content_area,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "the renderer needs the whole row context in one call"
)]
pub(super) fn draw_diff_side(
    frame: &mut Frame<'_>,
    area: Rect,
    line: Option<&DiffLine>,
    old_side: bool,
    horizontal_scroll: usize,
    emphasis: Option<&Range<usize>>,
    theme: &Theme,
) {
    let Some(line) = line else {
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().bg(theme.panel_alt)),
            area,
        );
        return;
    };
    let number = if old_side {
        line.old_line
    } else {
        line.new_line
    };
    let number = number.map_or(String::new(), |number| number.to_string());
    let (marker, marker_style) = marker_for(line.kind, theme);
    let mut spans = vec![
        Span::styled(format!("{number:>4} "), Style::default().fg(theme.muted)),
        Span::styled(marker, marker_style),
    ];
    spans.extend(highlight_spans(
        &line.spans,
        horizontal_scroll,
        area.width.saturating_sub(7) as usize,
        line.kind,
        emphasis,
        theme,
    ));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(line_background(line.kind, theme)),
        area,
    );
}

pub(super) fn highlight_spans<'a>(
    spans: &'a [HighlightSpan],
    horizontal_scroll: usize,
    width: usize,
    kind: DiffLineKind,
    emphasis: Option<&Range<usize>>,
    theme: &Theme,
) -> Vec<Span<'a>> {
    let mut skip = horizontal_scroll;
    let mut remaining = width;
    let mut source_offset = 0;
    let mut output = Vec::new();
    for span in spans {
        if remaining == 0 {
            break;
        }
        let foreground = span.foreground.map_or_else(
            || line_foreground(kind, theme),
            |syntax| theme.syntax(syntax),
        );
        let mut style = Style::default().fg(foreground);
        if span.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if span.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }

        let span_start = source_offset;
        let span_end = span_start + span.text.len();
        let intersection = emphasis.and_then(|range| {
            let start = range.start.max(span_start);
            let end = range.end.min(span_end);
            (start < end).then_some(start..end)
        });
        if let Some(changed) = intersection {
            push_highlight_piece(
                &mut output,
                span.text
                    .get(..changed.start - span_start)
                    .unwrap_or_default(),
                style,
                false,
                kind,
                theme,
                &mut skip,
                &mut remaining,
            );
            push_highlight_piece(
                &mut output,
                span.text
                    .get(changed.start - span_start..changed.end - span_start)
                    .unwrap_or_default(),
                style,
                true,
                kind,
                theme,
                &mut skip,
                &mut remaining,
            );
            push_highlight_piece(
                &mut output,
                span.text
                    .get(changed.end - span_start..)
                    .unwrap_or_default(),
                style,
                false,
                kind,
                theme,
                &mut skip,
                &mut remaining,
            );
        } else {
            push_highlight_piece(
                &mut output,
                &span.text,
                style,
                false,
                kind,
                theme,
                &mut skip,
                &mut remaining,
            );
        }
        source_offset = span_end;
    }
    output
}

#[expect(
    clippy::too_many_arguments,
    reason = "the renderer needs the whole row context in one call"
)]
pub(super) fn push_highlight_piece(
    output: &mut Vec<Span<'_>>,
    text: &str,
    mut style: Style,
    emphasized: bool,
    kind: DiffLineKind,
    theme: &Theme,
    skip: &mut usize,
    remaining: &mut usize,
) {
    if text.is_empty() || *remaining == 0 {
        return;
    }
    let text_width = text.width();
    if *skip >= text_width {
        *skip -= text_width;
        return;
    }
    if emphasized {
        style = match kind {
            DiffLineKind::Added => style.bg(theme.added_emphasis_background),
            DiffLineKind::Removed => style.bg(theme.removed_emphasis_background),
            _ => style,
        };
    }
    let sliced = slice_width(text, *skip, *remaining);
    *skip = 0;
    *remaining = remaining.saturating_sub(sliced.width());
    output.push(Span::styled(sliced, style));
}

pub(super) fn marker_for(kind: DiffLineKind, theme: &Theme) -> (&'static str, Style) {
    match kind {
        DiffLineKind::Added => ("  ", Style::default().fg(theme.added)),
        DiffLineKind::Removed => ("  ", Style::default().fg(theme.removed)),
        DiffLineKind::HunkHeader => (
            "  ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        DiffLineKind::Context | DiffLineKind::Meta => ("  ", Style::default().fg(theme.muted)),
        DiffLineKind::FileHeader | DiffLineKind::FileFooter => {
            ("", Style::default().fg(theme.muted))
        }
    }
}

pub(super) fn line_background(kind: DiffLineKind, theme: &Theme) -> Style {
    match kind {
        DiffLineKind::Added => Style::default().bg(theme.added_background),
        DiffLineKind::Removed => Style::default().bg(theme.removed_background),
        DiffLineKind::HunkHeader => Style::default().bg(theme.panel_alt).fg(theme.accent),
        _ => Style::default().bg(theme.panel),
    }
}

pub(super) const fn line_foreground(kind: DiffLineKind, theme: &Theme) -> Color {
    match kind {
        DiffLineKind::Added => theme.added,
        DiffLineKind::Removed => theme.removed,
        DiffLineKind::HunkHeader => theme.accent,
        _ => theme.text,
    }
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
pub(super) fn draw_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    offset: usize,
    length: usize,
    theme: &Theme,
) {
    if length <= area.height as usize || area.width == 0 {
        return;
    }
    let height = area.height as usize;
    let thumb_height = (height * height / length).max(1).min(height);
    let max_offset = length.saturating_sub(height).max(1);
    let thumb_start = offset.min(max_offset) * (height - thumb_height) / max_offset;
    for row in thumb_start..thumb_start + thumb_height {
        frame.render_widget(
            Paragraph::new("▐").style(Style::default().fg(theme.accent_soft)),
            Rect::new(area.right().saturating_sub(1), area.y + cells(row), 1, 1),
        );
    }
}

#[expect(
    clippy::option_if_let_else,
    reason = "the branch is one arm of a longer chain that map_or_else cannot express"
)]
pub(super) fn draw_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
    project_hits: &mut Vec<Rect>,
) {
    let left = if let Some(busy) = app.busy.as_deref() {
        Line::from(vec![
            Span::styled(
                format!(" {} ", app.operation_spinner()),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(busy, Style::default().fg(theme.text)),
        ])
    } else if let Some(progress) = app.pull_request_progress {
        Line::from(vec![
            Span::styled(
                format!(" {} ", app.operation_spinner()),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", progress.label()),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                progress_bar(progress.percent(), 12),
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("  {}%", progress.percent()),
                Style::default().fg(theme.muted),
            ),
        ])
    } else if app.refreshing {
        Line::from(Span::styled(
            " Refreshing repository…",
            Style::default().fg(theme.muted),
        ))
    } else {
        let branch = if app.status.branch.head.is_empty() {
            "—".to_owned()
        } else {
            app.status.branch.head.clone()
        };
        let summary = format!(
            "   {} changes   {} staged",
            app.status.changes.len(),
            app.status.staged_count()
        );
        let (worktree_gap, worktree_label) = if app.worktrees.len() > 1 {
            let mut label = String::new();
            label.push_str(&app.worktrees.len().to_string());
            label.push_str(" worktrees");
            let gap = "   ";
            let prefix_width = cells("  ".width())
                .saturating_add(cells(branch.width()))
                .saturating_add(cells(summary.width()))
                .saturating_add(cells(gap.width()));
            project_hits.push(clipped_link_area(
                area.x.saturating_add(prefix_width),
                area.y.saturating_add(1),
                label.width(),
                area,
            ));
            (gap, label)
        } else {
            ("", String::new())
        };
        Line::from(vec![
            Span::styled("  ", Style::default().fg(theme.accent)),
            link_span(
                branch.clone(),
                app.branch_open_target(&app.status.branch.head),
                clipped_link_area(
                    area.x.saturating_add(cells("  ".width())),
                    area.y.saturating_add(1),
                    branch.width(),
                    area,
                ),
                theme,
                link_hits,
            ),
            Span::styled(summary, Style::default().fg(theme.muted)),
            Span::raw(worktree_gap),
            Span::styled(
                worktree_label,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ])
    };
    frame.render_widget(
        Paragraph::new(left)
            .style(Style::default().bg(theme.panel_alt))
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(theme.border)),
            ),
        area,
    );
}
