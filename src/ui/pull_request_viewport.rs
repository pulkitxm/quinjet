#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn update_pull_request_diff_viewport(
    app: &mut App,
    side_by_side: bool,
    scroll: usize,
    height: usize,
    details_rows: usize,
) {
    if app.view != View::PullRequests
        || app.pull_request_section != PullRequestSection::Files
        || app.pull_request_file_view != crate::app::PullRequestFileView::AllFiles
    {
        return;
    }
    app.pull_request_scroll_anchor = if app.content_scroll >= details_rows {
        diff_row_line_index(app, side_by_side, scroll)
            .and_then(|index| app.preview_header_at(index))
            .and_then(file_header_path)
            .map(std::path::PathBuf::from)
            .and_then(|path| {
                let header_row = anchored_diff_row(app, side_by_side, &path)?;
                Some((path, scroll.saturating_sub(header_row), app.content_scroll))
            })
    } else {
        None
    };
    let mut paths = Vec::new();
    for row in scroll..scroll.saturating_add(height.saturating_mul(2)) {
        if let Some(path) = diff_row_line_index(app, side_by_side, row)
            .and_then(|index| app.preview_header_at(index))
            .and_then(file_header_path)
        {
            let path = std::path::PathBuf::from(path);
            if paths.last() != Some(&path) {
                paths.push(path);
            }
        }
    }
    app.pull_request_viewport_paths = paths;
}

pub(super) fn diff_row_line_index(app: &App, side_by_side: bool, row: usize) -> Option<usize> {
    if side_by_side {
        app.side_by_side_diff_rows
            .get(row)
            .and_then(|row| match row {
                SideBySideRow::FileHeader(index) | SideBySideRow::Full { index, .. } => {
                    Some(*index)
                }
                SideBySideRow::Split(old, new) => old.or(*new),
                SideBySideRow::FileFooter => None,
            })
    } else {
        app.unified_diff_rows.get(row).copied()
    }
}
