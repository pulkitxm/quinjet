#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_unified_diff(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    rows: &[usize],
    diff_scroll: usize,
    theme: &Theme,
) -> Vec<ContentFileHit> {
    let first_index = rows.get(diff_scroll).copied().unwrap_or_default();
    let mut in_file = inside_file_before(&app.document, first_index);
    let emphasis = visible_intraline_emphasis(
        &app.document.lines,
        rows.iter()
            .copied()
            .skip(diff_scroll)
            .take(area.height as usize),
    );
    let sticky = app
        .document
        .lines
        .get(first_index)
        .filter(|line| line.kind != DiffLineKind::FileHeader)
        .and_then(|_| sticky_file_header(&app.document, first_index));
    let content_y = area.y + u16::from(sticky.is_some());
    let content_height = area.height.saturating_sub(u16::from(sticky.is_some()));
    let mut hits = Vec::new();
    if let Some(header) = sticky {
        let sticky_area = Rect::new(area.x, area.y, area.width, 1);
        draw_file_header(frame, sticky_area, header, app, theme);
        if let Some(path) = file_header_path(header) {
            hits.push(ContentFileHit {
                area: sticky_area,
                path: path.into(),
            });
        }
    }
    for (offset, line_index) in rows
        .iter()
        .copied()
        .skip(diff_scroll)
        .take(content_height as usize)
        .enumerate()
    {
        let Some(line) = app.document.lines.get(line_index) else {
            continue;
        };
        let row_area = Rect::new(area.x, content_y + cells(offset), area.width, 1);
        match line.kind {
            DiffLineKind::FileHeader => {
                draw_file_header(frame, row_area, line, app, theme);
                if let Some(path) = file_header_path(line) {
                    hits.push(ContentFileHit {
                        area: row_area,
                        path: path.into(),
                    });
                }
                in_file = true;
            }
            DiffLineKind::FileFooter => {
                draw_file_footer(frame, row_area, theme);
                in_file = false;
            }
            _ => draw_unified_line(
                frame,
                row_area,
                line,
                in_file,
                app.horizontal_scroll,
                emphasis.get(&line_index),
                review_line_selected(app, line, None),
                theme,
            ),
        }
    }
    hits
}

#[expect(
    clippy::too_many_arguments,
    reason = "the renderer needs the whole row context in one call"
)]
pub(super) fn draw_unified_line(
    frame: &mut Frame<'_>,
    area: Rect,
    line: &DiffLine,
    _boxed: bool,
    horizontal_scroll: usize,
    emphasis: Option<&Range<usize>>,
    selected: bool,
    theme: &Theme,
) {
    let content_area = area;
    let old = line
        .old_line
        .map_or(String::new(), |number| number.to_string());
    let new = line
        .new_line
        .map_or(String::new(), |number| number.to_string());
    let (marker, marker_style) = marker_for(line.kind, theme);
    let mut spans = vec![
        Span::styled(format!("{old:>4} "), Style::default().fg(theme.muted)),
        Span::styled(format!("{new:>4} "), Style::default().fg(theme.muted)),
        Span::styled(marker, marker_style),
    ];
    spans.extend(highlight_spans(
        &line.spans,
        horizontal_scroll,
        content_area.width.saturating_sub(12) as usize,
        line.kind,
        emphasis,
        theme,
    ));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(line_background(line.kind, selected, theme)),
        content_area,
    );
}

pub(super) fn inside_file_before(document: &DiffDocument, offset: usize) -> bool {
    let mut in_file = false;
    for line in document.lines.iter().take(offset) {
        match line.kind {
            DiffLineKind::FileHeader => in_file = true,
            DiffLineKind::FileFooter => in_file = false,
            _ => {}
        }
    }
    in_file
}

pub(super) fn sticky_file_header(document: &DiffDocument, line_index: usize) -> Option<&DiffLine> {
    let mut header = None;
    for line in document.lines.iter().take(line_index.saturating_add(1)) {
        match line.kind {
            DiffLineKind::FileHeader => header = Some(line),
            DiffLineKind::FileFooter => header = None,
            _ => {}
        }
    }
    header
}

pub(super) fn file_header_path(line: &DiffLine) -> Option<&str> {
    line.spans
        .first()
        .map(|span| span.text.split("  · ").next().unwrap_or(span.text.as_str()))
}

pub(super) fn draw_file_header(
    frame: &mut Frame<'_>,
    area: Rect,
    line: &DiffLine,
    app: &App,
    theme: &Theme,
) {
    if area.width == 0 {
        return;
    }
    let label = line
        .spans
        .first()
        .map(|span| span.text.as_str())
        .unwrap_or_default();
    let disclosure = if app.preview_files_collapsible() {
        disclosure_glyph(
            file_header_path(line).is_none_or(|path| !app.preview_file_collapsed(path)),
        )
    } else {
        " "
    };
    let additions = line.spans.get(1).map_or("+0", |span| span.text.as_str());
    let deletions = line.spans.get(2).map_or("-0", |span| span.text.as_str());
    let icon = file_icon_span(Path::new(file_header_path(line).unwrap_or(label)), theme);
    let reserved = 10_usize + additions.width() + deletions.width();
    let label = truncate_middle(label, (area.width as usize).saturating_sub(reserved));
    let fill = (area.width as usize)
        .saturating_sub(reserved)
        .saturating_sub(label.width());
    let selected = file_header_path(line).is_some_and(|path| app.preview_file_selected(path));
    let background = if selected {
        theme.selected
    } else {
        theme.panel_alt
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("─", Style::default().fg(theme.border)),
            Span::styled(format!(" {disclosure} "), Style::default().fg(theme.muted)),
            icon,
            Span::raw(" "),
            Span::styled(
                label,
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled("─".repeat(fill), Style::default().fg(theme.border)),
            Span::styled(" ", Style::default()),
            Span::styled(
                additions.to_owned(),
                Style::default()
                    .fg(theme.added)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", Style::default()),
            Span::styled(
                deletions.to_owned(),
                Style::default()
                    .fg(theme.removed)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ─", Style::default().fg(theme.border)),
        ]))
        .style(Style::default().bg(background)),
        area,
    );
}

pub(super) fn draw_file_footer(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    if area.width == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize))
            .style(Style::default().fg(theme.border).bg(theme.panel)),
        area,
    );
}
