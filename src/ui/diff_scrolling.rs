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
        let y = area.y.saturating_add(cells(row.saturating_sub(scroll)));
        if side_by_side {
            match app.side_by_side_diff_rows.get(row).copied() {
                Some(SideBySideRow::Split(old, new)) => {
                    register_overflow(app, old, Rect::new(area.x, y, left_width, 1), 7);
                    register_overflow(
                        app,
                        new,
                        Rect::new(area.x + left_width + 1, y, right_width, 1),
                        7,
                    );
                }
                Some(SideBySideRow::Full { index, .. }) => {
                    register_overflow(app, Some(index), Rect::new(area.x, y, area.width, 1), 2);
                }
                _ => {}
            }
        } else {
            register_overflow(
                app,
                app.unified_diff_rows.get(row).copied(),
                Rect::new(area.x, y, area.width, 1),
                12,
            );
        }
    }
    app.horizontal_scroll = app.horizontal_scroll.min(app.horizontal_scroll_limit);
    if app.horizontal_scroll_limit == 0 {
        app.horizontal_scroll_regions.clear();
    }
}

fn register_overflow(app: &mut App, index: Option<usize>, area: Rect, gutter: u16) {
    let overflow = line_overflow(app, index, area.width.saturating_sub(gutter));
    let unwrapped_code = !app.wrap_code
        && index
            .and_then(|index| app.document.lines.get(index))
            .is_some_and(diff_wrapping::code_line);
    if overflow > 0 || unwrapped_code {
        app.horizontal_scroll_regions.push(Rect::new(
            area.x.saturating_add(gutter),
            area.y,
            area.width.saturating_sub(gutter),
            1,
        ));
        app.horizontal_scroll_limit = app.horizontal_scroll_limit.max(overflow);
    }
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
