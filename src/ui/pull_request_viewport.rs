#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn update_pull_request_diff_viewport(
    app: &mut App,
    side_by_side: bool,
    scroll: usize,
    height: usize,
) {
    if app.view != View::PullRequests
        || app.pull_request_section != PullRequestSection::Files
        || app.pull_request_file_view != crate::app::PullRequestFileView::AllFiles
    {
        return;
    }
    let mut paths = Vec::new();
    for row in scroll..scroll.saturating_add(height.saturating_mul(2)) {
        let index = if side_by_side {
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
        };
        if let Some(path) = index
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
