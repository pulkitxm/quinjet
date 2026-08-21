#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "rendering and hit registration share the same scrolled coordinate space"
)]
pub(super) fn draw_pull_request_details_scrolled(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    scroll: usize,
    total_rows: usize,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) {
    let Some(details) = app.document.pull_request_details.as_ref() else {
        return;
    };
    let block = Block::default()
        .title(format!(" Pull request #{} · details ", details.number))
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.panel_alt).fg(theme.text));
    let full_area = Rect::new(0, 0, area.width, cells(total_rows));
    let mut buffer = Buffer::empty(full_area);
    let inner = block.inner(full_area);
    block.render(full_area, &mut buffer);
    let state = if details.is_draft {
        "DRAFT"
    } else {
        details.state.as_str()
    };
    let head_repository = details.head_repository.as_deref().unwrap_or("deleted fork");
    let pull_request_target =
        (!details.url.is_empty()).then(|| OpenTarget::Browser(details.url.clone()));
    let title_width = details.title.width();
    let mut lines = vec![Line::from(link_span(
        details.title.clone(),
        pull_request_target,
        scrolled_detail_link_area(area, scroll, inner.y, inner.x, title_width),
        theme,
        link_hits,
    ))];
    let status_prefix = format!("{state}  ·  ");
    let author = format!("@{}", details.author);
    lines.push(scrolled_link_detail_line(
        "Status",
        status_prefix,
        author,
        format!(
            "  ·  updated {}",
            format_local_timestamp(&details.updated_at)
        ),
        app.account_open_target(&details.author),
        area,
        scroll,
        inner,
        1,
        theme,
        link_hits,
    ));
    for (index, description) in description_preview_lines(
        &details.description,
        inner.width.saturating_sub(12) as usize,
        3,
    )
    .into_iter()
    .enumerate()
    {
        lines.push(detail_line(
            if index == 0 { "Description" } else { "" },
            description,
            theme,
        ));
    }
    let mut selected_file = vec![Span::styled(
        format!("{:<DETAIL_LABEL_WIDTH$}", "Selected"),
        Style::default().fg(theme.muted),
    )];
    if let Some(path) = details.selected_file.as_deref() {
        selected_file.extend([
            file_icon_span(Path::new(path), theme),
            Span::raw(" "),
            Span::styled(path, Style::default().fg(theme.text)),
        ]);
    } else {
        selected_file.push(Span::styled(
            "Preparing files",
            Style::default().fg(theme.text),
        ));
    }
    selected_file.extend([
        Span::raw("  "),
        Span::styled(
            format!("+{}", details.selected_file_additions),
            Style::default()
                .fg(theme.added)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("-{}", details.selected_file_deletions),
            Style::default()
                .fg(theme.removed)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let source_row = lines.len();
    lines.push(scrolled_link_detail_line(
        "Source",
        String::new(),
        format!("{head_repository}:{}", details.head_ref),
        format!(
            "{}{}",
            remote_suffix(&details.head_remotes),
            if details.is_cross_repository {
                "  ·  fork"
            } else {
                ""
            }
        ),
        app.pull_request_head_branch_open_target(),
        area,
        scroll,
        inner,
        source_row,
        theme,
        link_hits,
    ));
    let destination_row = lines.len();
    lines.push(scrolled_link_detail_line(
        "Destination",
        String::new(),
        format!("{}:{}", details.base_repository, details.base_ref),
        remote_suffix(&details.base_remotes),
        app.pull_request_base_branch_open_target(),
        area,
        scroll,
        inner,
        destination_row,
        theme,
        link_hits,
    ));
    let url_row = lines.len();
    lines.push(scrolled_link_detail_line(
        "URL",
        String::new(),
        details.url.clone(),
        String::new(),
        (!details.url.is_empty()).then(|| OpenTarget::Browser(details.url.clone())),
        area,
        scroll,
        inner,
        url_row,
        theme,
        link_hits,
    ));
    lines.extend([
        Line::from(selected_file),
        Line::from(vec![
            Span::styled("PR total   ", Style::default().fg(theme.muted)),
            Span::styled(
                format!(
                    "{} file{} changed  ",
                    details.changed_files,
                    if details.changed_files == 1 { "" } else { "s" }
                ),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("+{}", details.additions),
                Style::default()
                    .fg(theme.added)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("-{}", details.deletions),
                Style::default()
                    .fg(theme.removed)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ]);
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

#[expect(
    clippy::too_many_arguments,
    reason = "the helper maps one linked detail value into a scrolled card"
)]
pub(super) fn scrolled_link_detail_line(
    label: &str,
    prefix: String,
    text: String,
    suffix: String,
    target: Option<OpenTarget>,
    area: Rect,
    scroll: usize,
    inner: Rect,
    row: usize,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> Line<'static> {
    let link_x = inner
        .x
        .saturating_add(cells(DETAIL_LABEL_WIDTH))
        .saturating_add(cells(prefix.width()));
    let link_area = scrolled_detail_link_area(
        area,
        scroll,
        inner.y.saturating_add(cells(row)),
        link_x,
        text.width(),
    );
    Line::from(vec![
        Span::styled(
            format!("{label:<DETAIL_LABEL_WIDTH$}"),
            Style::default().fg(theme.muted),
        ),
        Span::styled(prefix, Style::default().fg(theme.text)),
        link_span(text, target, link_area, theme, link_hits),
        Span::styled(suffix, Style::default().fg(theme.text)),
    ])
}

pub(super) fn description_preview_lines(
    value: &str,
    width: usize,
    maximum_lines: usize,
) -> Vec<String> {
    let description = markdown_preview_text(value);
    text_preview_lines(
        if description.is_empty() {
            "No description provided"
        } else {
            &description
        },
        width,
        maximum_lines,
    )
}

pub(super) fn markdown_preview_text(value: &str) -> String {
    let mut output = String::new();
    for raw_line in value.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let heading = line.starts_with('#');
        let line = line.trim_start_matches('#').trim_start();
        let (line, bullet) = ["- ", "* ", "+ "]
            .into_iter()
            .find_map(|marker| line.strip_prefix(marker).map(|line| (line, true)))
            .unwrap_or((line, false));
        let line = strip_inline_markdown(line);
        if line.is_empty()
            || (heading
                && matches!(
                    line.to_ascii_lowercase().as_str(),
                    "summary" | "description" | "overview"
                ))
        {
            continue;
        }
        if !output.is_empty() {
            output.push_str(if bullet { " • " } else { " " });
        }
        output.push_str(&line);
    }
    output
}

pub(super) fn strip_inline_markdown(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '*' | '`'))
        .collect()
}

pub(super) fn text_preview_lines(value: &str, width: usize, maximum_lines: usize) -> Vec<String> {
    if maximum_lines == 0 {
        return Vec::new();
    }
    let width = width.max(1);
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let total_width = normalized.width();
    let mut lines = Vec::with_capacity(maximum_lines);
    let mut skipped = 0;
    while skipped < total_width && lines.len() < maximum_lines {
        while skipped < total_width && slice_width(&normalized, skipped, 1) == " " {
            skipped += 1;
        }
        let chunk = slice_width(&normalized, skipped, width);
        if chunk.is_empty() {
            break;
        }
        let reaches_end = skipped.saturating_add(chunk.width()) >= total_width;
        let (line, used) = if reaches_end {
            let used = chunk.width();
            (chunk, used)
        } else if let Some(space) = chunk.rfind(' ').filter(|space| *space > 0) {
            let line = chunk.get(..space).unwrap_or_default().to_owned();
            let used = line.width().saturating_add(1);
            (line, used)
        } else {
            let used = chunk.width();
            (chunk, used)
        };
        if used == 0 {
            break;
        }
        skipped = skipped.saturating_add(used);
        lines.push(line);
    }
    if skipped < total_width
        && let Some(last) = lines.last_mut()
    {
        *last = format!(
            "{}…",
            slice_width(last.trim_end(), 0, width.saturating_sub(1))
        );
    }
    while lines.len() < maximum_lines {
        lines.push(String::new());
    }
    lines
}

pub(super) fn remote_suffix(remotes: &[String]) -> String {
    if remotes.is_empty() {
        String::new()
    } else {
        format!(
            "  ·  remote{} {}",
            if remotes.len() == 1 { "" } else { "s" },
            remotes.join(", ")
        )
    }
}

pub(super) fn detail_line<'a>(label: &'a str, value: String, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{label:<DETAIL_LABEL_WIDTH$}"),
            Style::default().fg(theme.muted),
        ),
        Span::styled(value, Style::default().fg(theme.text)),
    ])
}

pub(super) fn link_detail_line<'a>(label: &'a str, value: String, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{label:<DETAIL_LABEL_WIDTH$}"),
            Style::default().fg(theme.muted),
        ),
        Span::styled(value, Link::style(theme)),
    ])
}

pub(super) fn link_style(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

pub(super) fn link_span(
    text: String,
    target: Option<OpenTarget>,
    area: Rect,
    theme: &Theme,
    hits: &mut Vec<LinkHit>,
) -> Span<'static> {
    match target {
        None => Span::styled(
            text,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Some(target) => Link::new(target).span(text, area, theme, hits),
    }
}

pub(super) fn clipped_link_area(x: u16, y: u16, width: usize, container: Rect) -> Rect {
    if y < container.y || y >= container.bottom() || x >= container.right() {
        return Rect::default();
    }
    Rect::new(
        x.max(container.x),
        y,
        cells(width).min(container.right().saturating_sub(x.max(container.x))),
        1,
    )
}

pub(super) fn horizontally_scrolled_link_area(
    container: Rect,
    scroll: usize,
    start: usize,
    width: usize,
) -> Rect {
    let end = start.saturating_add(width);
    if end <= scroll {
        return Rect::default();
    }
    let visible_start = start.max(scroll);
    clipped_link_area(
        container
            .x
            .saturating_add(cells(visible_start.saturating_sub(scroll))),
        container.y,
        end.saturating_sub(visible_start),
        container,
    )
}

pub(super) fn scrolled_detail_link_area(
    area: Rect,
    scroll: usize,
    source_y: u16,
    source_x: u16,
    width: usize,
) -> Rect {
    let scroll = cells(scroll);
    if source_y < scroll {
        return Rect::default();
    }
    let row = source_y - scroll;
    clipped_link_area(
        area.x.saturating_add(source_x),
        area.y.saturating_add(row),
        width,
        area,
    )
}

pub(super) fn pull_request_reference(text: &str) -> Option<(usize, usize, u64)> {
    text.match_indices('#').find_map(|(start, _)| {
        let rest = text.get(start.saturating_add(1)..)?;
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let end = start.saturating_add(1).saturating_add(digits);
        let number = text.get(start.saturating_add(1)..end)?.parse().ok()?;
        Some((start, end, number))
    })
}

pub(super) fn unified_row_indices(document: &DiffDocument, app: &App) -> Vec<usize> {
    let mut rows = Vec::new();
    let mut index = 0;
    while let Some(line) = document.lines.get(index) {
        if line.kind == DiffLineKind::HunkHeader {
            index += 1;
            continue;
        }
        rows.push(index);
        let collapsed = line.kind == DiffLineKind::FileHeader
            && file_header_path(line).is_some_and(|path| app.preview_file_collapsed(path));
        index += 1;
        if collapsed {
            while document
                .lines
                .get(index)
                .is_some_and(|line| line.kind != DiffLineKind::FileFooter)
            {
                index += 1;
            }
            if index < document.lines.len() {
                index += 1;
            }
        }
    }
    rows
}
