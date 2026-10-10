#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::integer_division,
    reason = "pane sizes use whole terminal cells"
)]
pub(super) fn update_diff_scroll(app: &mut App, area: Rect, side_by_side: bool, scroll: usize) {
    let sticky = pull_request_viewport::diff_row_line_index(app, side_by_side, scroll)
        .and_then(|index| {
            app.document
                .lines
                .get(index)
                .filter(|line| line.kind != DiffLineKind::FileHeader)
                .and_then(|_| app.preview_header_at(index))
        })
        .is_some();
    let area = Rect::new(
        area.x,
        area.y.saturating_add(u16::from(sticky)),
        area.width,
        area.height.saturating_sub(u16::from(sticky)),
    );
    let left_width = area
        .width
        .saturating_sub(1)
        .saturating_mul(app.diff_split_percent)
        / 100;
    let right_width = area.width.saturating_sub(1).saturating_sub(left_width);
    for row in scroll..scroll.saturating_add(usize::from(area.height)) {
        let overflow =
            if side_by_side {
                app.side_by_side_diff_rows
                    .get(row)
                    .map_or(0, |row| match row {
                        SideBySideRow::Split(old, new) => {
                            line_overflow(app, *old, left_width.saturating_sub(7))
                                .max(line_overflow(app, *new, right_width.saturating_sub(7)))
                        }
                        SideBySideRow::Full { index, .. } => {
                            line_overflow(app, Some(*index), area.width.saturating_sub(2))
                        }
                        SideBySideRow::FileHeader(_) | SideBySideRow::FileFooter => 0,
                    })
            } else {
                line_overflow(
                    app,
                    app.unified_diff_rows.get(row).copied(),
                    area.width.saturating_sub(12),
                )
            };
        if overflow > 0 {
            app.horizontal_scroll_regions.push(Rect::new(
                area.x,
                area.y.saturating_add(cells(row.saturating_sub(scroll))),
                area.width,
                1,
            ));
            app.horizontal_scroll_limit = app.horizontal_scroll_limit.max(overflow);
        }
    }
    app.horizontal_scroll = app.horizontal_scroll.min(app.horizontal_scroll_limit);
}

fn line_overflow(app: &App, index: Option<usize>, width: u16) -> usize {
    index
        .and_then(|index| app.document.lines.get(index))
        .map_or(0, |line| {
            if app.wrap_code && diff_wrapping::code_line(line) {
                return 0;
            }
            if matches!(
                line.kind,
                DiffLineKind::FileHeader | DiffLineKind::FileFooter | DiffLineKind::Image
            ) {
                return 0;
            }
            line.spans
                .iter()
                .map(|span| span.text.width())
                .sum::<usize>()
                .saturating_sub(usize::from(width))
        })
}
