#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn project_header_line(
    group: &ProjectGroup,
    expanded: bool,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let button = if expanded { "[⌄]" } else { "[›]" };
    let updated = group
        .updated_at()
        .map_or_else(|| "no commits".to_owned(), format_relative_timestamp);
    let fixed = 6_usize.saturating_add(updated.width());
    let name = truncate_middle(&group.name, width.saturating_sub(fixed));
    let used = 5_usize
        .saturating_add(name.width())
        .saturating_add(updated.width());
    let padding = " ".repeat(width.saturating_sub(used));
    Line::from(vec![
        Span::styled(" ", Style::default().bg(theme.panel)),
        Span::styled(
            button,
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel_alt)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {name}"),
            Style::default()
                .fg(theme.muted)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(padding, Style::default().bg(theme.panel)),
        Span::styled(updated, Style::default().fg(theme.muted).bg(theme.panel)),
    ])
}

pub(super) fn project_worktree_line(
    tree: &crate::git::Worktree,
    active: bool,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let background = if active { theme.selected } else { theme.panel };
    let flag = if tree.current {
        "this session"
    } else if tree.locked.is_some() {
        "locked"
    } else if tree.prunable.is_some() {
        "prunable"
    } else {
        ""
    };
    let updated = tree
        .updated_at
        .as_deref()
        .map_or_else(|| "no commits".to_owned(), format_relative_timestamp);
    let metadata = if flag.is_empty() {
        updated
    } else {
        format!("{flag}  {updated}")
    };
    let left_width = width.saturating_sub(metadata.width().saturating_add(1));
    let branch_width = left_width.saturating_sub(5).min(26);
    let branch = truncate_middle(&format!("- {}", tree.branch_label()), branch_width);
    let path_width = left_width
        .saturating_sub(3_usize.saturating_add(branch.width()).saturating_add(2))
        .min(34);
    let path = truncate_middle(&tree.path.display().to_string(), path_width);
    let rendered_path_width = if path.is_empty() { 0 } else { 2 + path.width() };
    let used = 3_usize
        .saturating_add(branch.width())
        .saturating_add(rendered_path_width)
        .saturating_add(metadata.width());
    let padding = " ".repeat(width.saturating_sub(used));
    Line::from(vec![
        Span::styled(
            if tree.current { " ● " } else { "   " },
            Style::default()
                .fg(if tree.current {
                    theme.success
                } else {
                    theme.muted
                })
                .bg(background),
        ),
        Span::styled(
            branch,
            Style::default()
                .fg(theme.text)
                .bg(background)
                .add_modifier(if active {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
        Span::styled(
            if path.is_empty() {
                String::new()
            } else {
                format!("  {path}")
            },
            Style::default().fg(theme.muted).bg(background),
        ),
        Span::styled(padding, Style::default().bg(background)),
        Span::styled(metadata, Style::default().fg(theme.muted).bg(background)),
    ])
}
