#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::integer_division,
    reason = "pane sizes use whole terminal cells"
)]
pub(super) fn wrap_diff_rows(app: &mut App, side_by_side: bool, width: u16) {
    app.diff_wrap_offsets.clear();
    if !app.wrap_code {
        return;
    }
    if side_by_side {
        let left = width
            .saturating_sub(1)
            .saturating_mul(app.diff_split_percent)
            / 100;
        let right = width.saturating_sub(1).saturating_sub(left);
        let rows = std::mem::take(&mut app.side_by_side_diff_rows);
        for row in rows {
            if let SideBySideRow::Split(old, new) = row {
                let old_starts = wrap_starts(app, old, left.saturating_sub(7));
                let new_starts = wrap_starts(app, new, right.saturating_sub(7));
                for part in 0..old_starts.len().max(new_starts.len()) {
                    app.side_by_side_diff_rows.push(SideBySideRow::Split(
                        old.filter(|_| part < old_starts.len()),
                        new.filter(|_| part < new_starts.len()),
                    ));
                    app.diff_wrap_offsets.push((
                        old_starts.get(part).copied().unwrap_or_default(),
                        new_starts.get(part).copied().unwrap_or_default(),
                    ));
                }
            } else {
                app.side_by_side_diff_rows.push(row);
                app.diff_wrap_offsets.push((0, 0));
            }
        }
    } else {
        let rows = std::mem::take(&mut app.unified_diff_rows);
        for index in rows {
            for offset in wrap_starts(app, Some(index), width.saturating_sub(12)) {
                app.unified_diff_rows.push(index);
                app.diff_wrap_offsets.push((offset, offset));
            }
        }
    }
}

fn wrap_starts(app: &App, index: Option<usize>, width: u16) -> Vec<usize> {
    let Some(line) = index.and_then(|index| app.document.lines.get(index)) else {
        return vec![0];
    };
    if !code_line(line) {
        return vec![0];
    }
    let width = usize::from(width.max(2));
    let mut starts = vec![0];
    let mut consumed = 0_usize;
    let mut row_width = 0_usize;
    for character in line.spans.iter().flat_map(|span| span.text.chars()) {
        let character_width = character.width().unwrap_or_default();
        if row_width.saturating_add(character_width) > width {
            starts.push(consumed);
            row_width = 0;
        }
        row_width = row_width.saturating_add(character_width);
        consumed = consumed.saturating_add(character_width);
    }
    starts
}

pub(super) const fn code_line(line: &DiffLine) -> bool {
    matches!(
        line.kind,
        DiffLineKind::Added | DiffLineKind::Removed | DiffLineKind::Context
    )
}

pub(super) fn row_offset(app: &App, row: usize, old: bool) -> usize {
    app.diff_wrap_offsets
        .get(row)
        .map_or(0, |offset| if old { offset.0 } else { offset.1 })
}

pub(super) fn remember_scroll(app: &mut App, side_by_side: bool, row: usize) {
    app.diff_scroll_anchor = pull_request_viewport::diff_row_line_index(app, side_by_side, row)
        .map(|index| {
            (
                index,
                anchor_column(app, side_by_side, row),
                app.content_scroll,
            )
        });
}

pub(super) fn restore_scroll(app: &mut App, side_by_side: bool, details_rows: usize) {
    if !std::mem::take(&mut app.diff_restore_scroll) {
        return;
    }
    let Some((index, column, scroll)) = app.diff_scroll_anchor else {
        return;
    };
    if scroll != app.content_scroll {
        return;
    }
    let length = if side_by_side {
        app.side_by_side_diff_rows.len()
    } else {
        app.unified_diff_rows.len()
    };
    if let Some(row) = (0..length).rfind(|row| {
        pull_request_viewport::diff_row_line_index(app, side_by_side, *row) == Some(index)
            && anchor_column(app, side_by_side, *row) <= column
    }) {
        app.content_scroll = details_rows.saturating_add(row);
    }
}

fn anchor_column(app: &App, side_by_side: bool, row: usize) -> usize {
    let old = !side_by_side
        || !matches!(
            app.side_by_side_diff_rows.get(row),
            Some(SideBySideRow::Split(None, Some(_)))
        );
    row_offset(app, row, old)
}

pub(super) fn render_offset(app: &App, row: usize, old: bool) -> usize {
    if app.wrap_code {
        row_offset(app, row, old)
    } else {
        app.horizontal_scroll
    }
}
