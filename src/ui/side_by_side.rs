#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
pub(super) fn draw_side_by_side_diff(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    rows: &[SideBySideRow],
    diff_scroll: usize,
    theme: &Theme,
) -> (Rect, Vec<ContentFileHit>) {
    let content = area;
    let usable_width = content.width.saturating_sub(1);
    let left_width = usable_width.saturating_mul(app.diff_split_percent) / 100;
    let left = Rect::new(content.x, content.y, left_width, content.height);
    let divider = Rect::new(left.right(), area.y, 1, area.height);
    let right = Rect::new(
        divider.right(),
        area.y,
        content.right().saturating_sub(divider.right()),
        area.height,
    );
    let divider_color = if app.resize_target == Some(crate::app::ResizeTarget::Diff) {
        theme.border_focus
    } else {
        theme.border
    };

    let lines = &app.document.lines;
    let sticky = rows.get(diff_scroll).and_then(|first| match first {
        SideBySideRow::FileHeader(_) | SideBySideRow::FileFooter => None,
        _ => rows
            .get(..diff_scroll)
            .unwrap_or_default()
            .iter()
            .rev()
            .find_map(|row| match row {
                SideBySideRow::FileHeader(header) => lines.get(*header),
                _ => None,
            }),
    });
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
    for (offset, row) in rows
        .iter()
        .skip(diff_scroll)
        .take(content_height as usize)
        .enumerate()
    {
        let y = content_y + cells(offset);
        let row_area = Rect::new(area.x, y, area.width, 1);
        match row {
            SideBySideRow::FileHeader(header) => {
                let Some(line) = lines.get(*header) else {
                    continue;
                };
                draw_file_header(frame, row_area, line, app, theme);
                if let Some(path) = file_header_path(line) {
                    hits.push(ContentFileHit {
                        area: row_area,
                        path: path.into(),
                    });
                }
            }
            SideBySideRow::FileFooter => draw_file_footer(frame, row_area, theme),
            SideBySideRow::Full { index, boxed } => {
                let Some(line) = lines.get(*index) else {
                    continue;
                };
                draw_full_width_diff_line(
                    frame,
                    row_area,
                    line,
                    *boxed,
                    app.horizontal_scroll,
                    theme,
                );
            }
            SideBySideRow::Split(old_index, new_index) => {
                let old_line = old_index.and_then(|line_index| lines.get(line_index));
                let new_line = new_index.and_then(|line_index| lines.get(line_index));
                let (old_emphasis, new_emphasis) = paired_intraline_emphasis(old_line, new_line);
                draw_diff_side(
                    frame,
                    Rect::new(left.x, y, left.width, 1),
                    old_line,
                    true,
                    app.horizontal_scroll,
                    old_emphasis.as_ref(),
                    old_line.is_some_and(|line| review_line_selected(app, line, Some(true))),
                    theme,
                );
                frame.render_widget(
                    Paragraph::new("│").style(Style::default().fg(divider_color).bg(theme.panel)),
                    Rect::new(divider.x, y, 1, 1),
                );
                draw_diff_side(
                    frame,
                    Rect::new(right.x, y, right.width, 1),
                    new_line,
                    false,
                    app.horizontal_scroll,
                    new_emphasis.as_ref(),
                    new_line.is_some_and(|line| review_line_selected(app, line, Some(false))),
                    theme,
                );
            }
        }
    }
    (divider, hits)
}

pub(super) fn side_by_side_rows(document: &DiffDocument, app: &App) -> Vec<SideBySideRow> {
    let mut rows = Vec::new();
    let mut index = 0;
    let mut in_file = false;
    while let Some(line) = document.lines.get(index) {
        match line.kind {
            DiffLineKind::FileHeader => {
                rows.push(SideBySideRow::FileHeader(index));
                in_file = true;
                index += 1;
                if file_header_path(line).is_some_and(|path| app.preview_file_collapsed(path)) {
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
                    in_file = false;
                }
            }
            DiffLineKind::FileFooter => {
                rows.push(SideBySideRow::FileFooter);
                in_file = false;
                index += 1;
            }
            DiffLineKind::HunkHeader => {
                index += 1;
            }
            DiffLineKind::Meta | DiffLineKind::Review => {
                rows.push(SideBySideRow::Full {
                    index,
                    boxed: in_file,
                });
                index += 1;
            }
            DiffLineKind::Added => {
                rows.push(SideBySideRow::Split(None, Some(index)));
                index += 1;
            }
            DiffLineKind::Context => {
                rows.push(SideBySideRow::Split(Some(index), Some(index)));
                index += 1;
            }
            DiffLineKind::Removed => {
                let removed_start = index;
                while document
                    .lines
                    .get(index)
                    .is_some_and(|line| line.kind == DiffLineKind::Removed)
                {
                    index += 1;
                }
                let added_start = index;
                while document
                    .lines
                    .get(index)
                    .is_some_and(|line| line.kind == DiffLineKind::Added)
                {
                    index += 1;
                }
                let removed_len = added_start - removed_start;
                let added_len = index - added_start;
                for pair_index in 0..removed_len.max(added_len) {
                    rows.push(SideBySideRow::Split(
                        (pair_index < removed_len).then(|| removed_start + pair_index),
                        (pair_index < added_len).then(|| added_start + pair_index),
                    ));
                }
            }
        }
    }
    rows
}

pub(super) struct EmphasisBlock {
    removed_start: usize,
    added_start: usize,
    added_end: usize,
}

impl EmphasisBlock {
    pub(super) const fn contains(&self, index: usize) -> bool {
        self.removed_start <= index && index < self.added_end
    }
}

pub(super) fn emphasis_run_start(
    lines: &[DiffLine],
    mut start: usize,
    kind: DiffLineKind,
) -> usize {
    while start > 0
        && lines
            .get(start.saturating_sub(1))
            .is_some_and(|line| line.kind == kind)
    {
        start = start.saturating_sub(1);
    }
    start
}

pub(super) fn emphasis_run_end(lines: &[DiffLine], mut end: usize, kind: DiffLineKind) -> usize {
    while lines.get(end).is_some_and(|line| line.kind == kind) {
        end = end.saturating_add(1);
    }
    end
}

pub(super) fn emphasis_block(lines: &[DiffLine], index: usize) -> Option<EmphasisBlock> {
    match lines.get(index)?.kind {
        DiffLineKind::Removed => {
            let removed_start = emphasis_run_start(lines, index, DiffLineKind::Removed);
            let added_start =
                emphasis_run_end(lines, index.saturating_add(1), DiffLineKind::Removed);
            let added_end = emphasis_run_end(lines, added_start, DiffLineKind::Added);
            Some(EmphasisBlock {
                removed_start,
                added_start,
                added_end,
            })
        }
        DiffLineKind::Added => {
            let added_start = emphasis_run_start(lines, index, DiffLineKind::Added);
            let removed_start = emphasis_run_start(lines, added_start, DiffLineKind::Removed);
            let added_end = emphasis_run_end(lines, index.saturating_add(1), DiffLineKind::Added);
            Some(EmphasisBlock {
                removed_start,
                added_start,
                added_end,
            })
        }
        _ => None,
    }
}

pub(super) fn visible_intraline_emphasis(
    lines: &[DiffLine],
    visible: impl Iterator<Item = usize>,
) -> HashMap<usize, Range<usize>> {
    let mut emphasis = HashMap::new();
    let mut block: Option<EmphasisBlock> = None;
    for index in visible {
        let Some(kind) = lines.get(index).map(|line| line.kind) else {
            continue;
        };
        if kind != DiffLineKind::Removed && kind != DiffLineKind::Added {
            continue;
        }
        if !block.as_ref().is_some_and(|block| block.contains(index)) {
            block = emphasis_block(lines, index);
        }
        let Some(current) = block.as_ref() else {
            continue;
        };
        let pair_count = current
            .added_start
            .saturating_sub(current.removed_start)
            .min(current.added_end.saturating_sub(current.added_start));
        let pair_index = if kind == DiffLineKind::Removed {
            index.saturating_sub(current.removed_start)
        } else {
            index.saturating_sub(current.added_start)
        };
        if pair_index >= pair_count {
            continue;
        }
        let (old_range, new_range) = paired_intraline_emphasis(
            lines.get(current.removed_start.saturating_add(pair_index)),
            lines.get(current.added_start.saturating_add(pair_index)),
        );
        let range = if kind == DiffLineKind::Removed {
            old_range
        } else {
            new_range
        };
        if let Some(range) = range {
            let _ = emphasis.insert(index, range);
        }
    }
    emphasis
}

pub(super) fn paired_intraline_emphasis(
    old_line: Option<&DiffLine>,
    new_line: Option<&DiffLine>,
) -> (Option<Range<usize>>, Option<Range<usize>>) {
    let (Some(old_line), Some(new_line)) = (old_line, new_line) else {
        return (None, None);
    };
    if old_line.kind != DiffLineKind::Removed || new_line.kind != DiffLineKind::Added {
        return (None, None);
    }
    let old_bytes = old_line
        .spans
        .iter()
        .fold(0_usize, |total, span| total.saturating_add(span.text.len()));
    let new_bytes = new_line
        .spans
        .iter()
        .fold(0_usize, |total, span| total.saturating_add(span.text.len()));
    if old_bytes.max(new_bytes) > MAX_INTRALINE_SOURCE_BYTES {
        return (None, None);
    }
    changed_ranges(&old_line.text(), &new_line.text())
}

pub(super) fn changed_ranges(old: &str, new: &str) -> (Option<Range<usize>>, Option<Range<usize>>) {
    let mut prefix = 0;
    for ((old_index, old_character), (new_index, new_character)) in
        old.char_indices().zip(new.char_indices())
    {
        if old_character != new_character || old_index != new_index {
            break;
        }
        prefix = old_index + old_character.len_utf8();
    }

    let mut old_end = old.len();
    let mut new_end = new.len();
    for ((old_index, old_character), (new_index, new_character)) in
        old.char_indices().rev().zip(new.char_indices().rev())
    {
        if old_character != new_character || old_index < prefix || new_index < prefix {
            break;
        }
        old_end = old_index;
        new_end = new_index;
    }

    (
        (prefix < old_end).then_some(prefix..old_end),
        (prefix < new_end).then_some(prefix..new_end),
    )
}
