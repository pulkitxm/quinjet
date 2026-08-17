mod theme;

use std::ops::Range;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use self::theme::Theme;
use crate::app::{
    App, ChangeRow, ChangeSection, ContentFileHit, ContentStepHit, DiffLayout, Focus, LinkHit,
    Modal, OpenTarget, PaletteCommand, PullRequestContentRow, PullRequestSection,
    PullRequestTreeEntry, ScmAction, ScmActionHit, SidebarHit, SidebarHitArea, ToastLevel,
    UiGeometry, View,
};
use crate::convert::cells;
use crate::date_time::format_local_timestamp;
use crate::git::diff::{DiffDocument, DiffLine, DiffLineKind, HighlightSpan, PullRequestDetails};
use crate::git::github::{
    CheckLogLine, CheckLogSeverity, CheckStep, ConversationEntry, ConversationKind,
    GitHubRepository, PullRequestCheckStatus, PullRequestFileStatus,
};
#[cfg(test)]
use crate::git::github::{PullRequestCheck, PullRequestFile};
use crate::git::status::{Change, ChangeArea, ChangeStatus};
use crate::git::{Branch, HistoryBranch, Stash};

const DETAIL_LABEL_WIDTH: usize = 12;
const MAX_INTRALINE_SOURCE_BYTES: usize = 32 * 1024;

const HELP_LINES: &[(&str, &str)] = &[
    ("Navigation", ""),
    ("j / k, ↑ / ↓", "Move selection or scroll preview"),
    (
        "Shift + drag",
        "Select terminal text without activating controls",
    ),
    ("Double-click divider", "Restore that pane's default size"),
    ("PgUp / PgDn", "Move by a page"),
    ("gg / G", "Jump to first / last item"),
    ("Tab", "Switch focus between sidebar and preview"),
    ("Enter", "Toggle sidebar / preview focus"),
    ("h / l, ← / →", "Scroll preview horizontally"),
    ("[ / ]", "Previous / next diff hunk"),
    ("e / E", "Collapse / expand multi-file diffs"),
    ("Space in preview", "Toggle a file in a multi-file preview"),
    ("z", "Hide / show sidebar"),
    ("1 / 2 / 3", "Changes / commit history / pull requests"),
    ("/", "Filter the active list"),
    ("Esc", "Clear filter, close modal, or return focus"),
    ("", ""),
    ("Changes", ""),
    ("s / u", "Stage / unstage selected file"),
    ("[+] / [−]", "Click an individual file or group action"),
    ("a / U", "Stage all / unstage all"),
    ("x", "Discard selected change (asks first)"),
    ("c", "Commit staged changes"),
    ("S", "View and manage stashes"),
    ("d", "Compare current branch with another branch"),
    ("b / B", "Branch picker / checkout branch picker"),
    ("", ""),
    ("History", ""),
    ("b", "View another local or remote branch (no checkout)"),
    ("C / R", "Cherry-pick / revert selected commit"),
    ("n", "Create branch at selected commit"),
    ("", ""),
    ("Pull Requests", ""),
    ("3", "Open the on-demand PR view (no automatic fetch)"),
    ("/", "Focus the numeric PR field; Enter opens it"),
    ("o", "Discover or choose the base repository"),
    (
        "Shift+P / Shift+F",
        "The PR and its checks / all changed files",
    ),
    (
        "j / k",
        "Select the conversation, a check, a file, or a folder",
    ),
    ("← / →, Enter", "Collapse / expand the selected folder"),
    ("r", "Refetch this PR now, bypassing the cache"),
    ("", ""),
    ("Check Logs", ""),
    ("j / k in the list", "Select a check to read its run log"),
    ("Tab, then j / k", "Move through that run's steps"),
    ("[ / ]", "Previous / next step"),
    ("Space, Enter", "Fold or unfold the selected step"),
    ("e / E", "Fold or unfold every step"),
    (
        "PgUp / PgDn, wheel",
        "Scroll the output of an unfolded step",
    ),
    ("h / l, ← / →", "Read a log line past the right edge"),
    ("", ""),
    ("Repository", ""),
    ("r / Ctrl+R", "Refresh"),
    ("f / l / p / y", "Fetch / pull / push / sync"),
    ("v", "Toggle unified / side-by-side diff"),
    (": / Ctrl+P", "Open command palette"),
    ("?", "Show this help"),
    ("q", "Quit"),
];

pub(crate) fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let theme = Theme::default();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.background)),
        frame.area(),
    );

    if frame.area().width < 72 || frame.area().height < 18 {
        draw_too_small(frame, &theme);
        return;
    }

    let [tabs, main, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(2),
    ])
    .areas(frame.area());
    let maximum_sidebar = main.width.saturating_sub(32).max(22);
    app.sidebar_width = app.sidebar_width.clamp(22, maximum_sidebar);
    let [sidebar_area, sidebar_divider, content_area] = if app.sidebar_hidden {
        [Rect::default(), Rect::default(), main]
    } else {
        Layout::horizontal([
            Constraint::Length(app.sidebar_width),
            Constraint::Length(1),
            Constraint::Min(31),
        ])
        .areas(main)
    };

    let (changes_tab, history_tab, pull_requests_tab, mut link_hits) =
        draw_tabs(frame, tabs, app, &theme);
    let (sidebar_hits, scm_action_hits) = if app.sidebar_hidden {
        (Vec::new(), Vec::new())
    } else {
        draw_sidebar(frame, sidebar_area, app, &theme, &mut link_hits)
    };
    if !app.sidebar_hidden {
        draw_main_divider(frame, sidebar_divider, app.resize_target.is_some(), &theme);
    }
    let (diff_divider, content_file_hits, content_step_hits) =
        draw_content(frame, content_area, app, &theme, &mut link_hits);
    draw_footer(frame, footer, app, &theme, &mut link_hits);

    app.geometry = UiGeometry {
        changes_tab,
        history_tab,
        pull_requests_tab,
        main,
        sidebar: sidebar_area,
        sidebar_divider,
        content: content_area,
        diff_divider,
        sidebar_hits,
        scm_action_hits,
        content_file_hits,
        content_step_hits,
        link_hits,
    };

    if let Some(modal) = app.modal.as_ref() {
        draw_modal(frame, modal, app, &theme);
    }
    if let Some(toast) = app.toast.as_ref() {
        draw_toast(frame, toast.message.as_str(), toast.level, &theme);
    }
}

fn draw_main_divider(frame: &mut Frame<'_>, area: Rect, dragging: bool, theme: &Theme) {
    let color = if dragging {
        theme.border_focus
    } else {
        theme.border
    };
    for row in area.y..area.bottom() {
        frame.render_widget(
            Paragraph::new("│").style(Style::default().fg(color).bg(theme.background)),
            Rect::new(area.x, row, 1, 1),
        );
    }
}

fn draw_too_small(frame: &mut Frame<'_>, theme: &Theme) {
    let text = Text::from(vec![
        Line::from(Span::styled(
            "Quinjet",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Terminal too small",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "Resize to at least 72 × 18",
            Style::default().fg(theme.muted),
        )),
    ]);
    frame.render_widget(
        Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border)),
        ),
        centered_rect(50, 8, frame.area()),
    );
}

fn draw_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
) -> (Rect, Rect, Rect, Vec<LinkHit>) {
    let mut link_hits = Vec::new();
    let path = app.repository_root.display().to_string();
    let show_path = path != app.repository_name;
    let branch = if app.status.branch.head.is_empty() {
        "detecting branch…".to_owned()
    } else {
        let mut text = format!(" {}", app.status.branch.head);
        if app.status.branch.ahead > 0 {
            text.push_str("  ↑");
            text.push_str(&app.status.branch.ahead.to_string());
        }
        if app.status.branch.behind > 0 {
            text.push_str("  ↓");
            text.push_str(&app.status.branch.behind.to_string());
        }
        text
    };
    let [
        changes_tab,
        history_tab,
        pull_requests_tab,
        title_area,
        branch_area,
    ] = Layout::horizontal([
        Constraint::Length(13),
        Constraint::Length(13),
        Constraint::Length(17),
        Constraint::Min(8),
        Constraint::Length(cells((branch.width() + 3).min(area.width as usize))),
    ])
    .areas(area);

    draw_tab(
        frame,
        changes_tab,
        "  Changes  ",
        app.view == View::Changes,
        theme,
    );
    draw_tab(
        frame,
        history_tab,
        "  History  ",
        app.view == View::History,
        theme,
    );
    draw_tab(
        frame,
        pull_requests_tab,
        " Pull Requests ",
        app.view == View::PullRequests,
        theme,
    );
    let prefix = " QUINJET ";
    let repository_x = title_area.x.saturating_add(cells(prefix.width()));
    let repository_area = clipped_link_area(
        repository_x,
        title_area.y.saturating_add(1),
        app.repository_name.width(),
        title_area,
    );
    let mut title = vec![
        Span::styled(
            prefix,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        interactive_span(
            app.repository_name.clone(),
            app.repository_open_target(),
            repository_area,
            theme,
            &mut link_hits,
        ),
    ];
    if show_path {
        let path_x = repository_x
            .saturating_add(cells(app.repository_name.width()))
            .saturating_add(2);
        title.push(Span::raw("  "));
        title.push(interactive_span(
            path.clone(),
            Some(app.workspace_open_target()),
            clipped_link_area(
                path_x,
                title_area.y.saturating_add(1),
                path.width(),
                title_area,
            ),
            theme,
            &mut link_hits,
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(title)).block(
            Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.panel)),
        ),
        title_area,
    );
    let branch_line = if app.status.branch.head.is_empty() {
        Line::from(branch.clone())
    } else {
        let branch_start = branch_area
            .right()
            .saturating_sub(1)
            .saturating_sub(cells(branch.width()));
        let branch_name_x = branch_start.saturating_add(cells(" ".width()));
        let branch_target = if app.status.branch.detached {
            app.status
                .branch
                .oid
                .as_deref()
                .and_then(|oid| app.commit_open_target(oid))
        } else {
            app.branch_open_target(&app.status.branch.head)
        };
        let branch_name = app.status.branch.head.clone();
        let branch_suffix = branch
            .strip_prefix(&format!(" {branch_name}"))
            .unwrap_or_default()
            .to_owned();
        Line::from(vec![
            Span::raw(" "),
            interactive_span(
                branch_name.clone(),
                branch_target,
                clipped_link_area(
                    branch_name_x,
                    branch_area.y.saturating_add(1),
                    branch_name.width(),
                    branch_area,
                ),
                theme,
                &mut link_hits,
            ),
            Span::raw(branch_suffix),
        ])
    };
    frame.render_widget(
        Paragraph::new(branch_line)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.accent).bg(theme.panel))
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(Style::default().fg(theme.border)),
            ),
        branch_area,
    );
    (changes_tab, history_tab, pull_requests_tab, link_hits)
}

fn draw_tab(frame: &mut Frame<'_>, area: Rect, label: &str, active: bool, theme: &Theme) {
    let style = if active {
        Style::default()
            .fg(theme.text)
            .bg(theme.accent_soft)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted).bg(theme.panel)
    };
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Center)
            .style(style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if active {
                        theme.border_focus
                    } else {
                        theme.border
                    })),
            ),
        area,
    );
}

fn draw_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> (Vec<SidebarHitArea>, Vec<ScmActionHit>) {
    match app.view {
        View::Changes => draw_changes_sidebar(frame, area, app, theme),
        View::History => (
            draw_history_sidebar(frame, area, app, theme, link_hits),
            Vec::new(),
        ),
        View::PullRequests => (
            draw_pull_requests_sidebar(frame, area, app, theme),
            Vec::new(),
        ),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
fn draw_changes_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> (Vec<SidebarHitArea>, Vec<ScmActionHit>) {
    let block = panel_block(
        if app.filter.is_empty() {
            format!(" Changes  {} ", app.status.changes.len())
        } else {
            format!(" Changes  /{} ", app.filter)
        },
        app.focus == Focus::Sidebar && app.modal.is_none(),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return (Vec::new(), Vec::new());
    }

    let controls_height = inner.height.min(3);
    let list_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(controls_height),
    );
    let visible = app.visible_change_indices();
    let rows = app.change_rows();
    let row_count = rows.len();
    let height = list_area.height as usize;
    let selected_row = rows
        .iter()
        .position(|row| match row {
            ChangeRow::Section { section, .. } => app.selected_change_section == Some(*section),
            ChangeRow::Change { cursor, .. } => {
                app.selected_change_section.is_none() && *cursor == app.change_cursor
            }
        })
        .unwrap_or_default();
    ensure_offset(&mut app.sidebar_offset, selected_row, height, row_count);
    let mut hits = Vec::new();
    let mut action_hits = Vec::new();
    let end = (app.sidebar_offset + height).min(rows.len());
    for (y, row) in
        (list_area.y..list_area.bottom()).zip(rows.iter().take(end).skip(app.sidebar_offset))
    {
        match row {
            ChangeRow::Section {
                section,
                count,
                collapsed,
            } => {
                let selected = app.selected_change_section == Some(*section);
                let background = if selected {
                    theme.selected
                } else {
                    theme.panel_alt
                };
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            if *collapsed { " ▸ " } else { " ▾ " },
                            Style::default().fg(theme.muted),
                        ),
                        Span::styled(
                            section.label().to_uppercase(),
                            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("  {count}"), Style::default().fg(theme.muted)),
                    ]))
                    .style(Style::default().bg(background)),
                    Rect::new(list_area.x, y, list_area.width, 1),
                );
                let (label, action) = match section {
                    ChangeSection::Staged => ("[−]", ScmAction::UnstageSection(*section)),
                    ChangeSection::Conflict
                    | ChangeSection::Unstaged
                    | ChangeSection::Untracked => ("[+]", ScmAction::StageSection(*section)),
                };
                let action_area = Rect::new(list_area.right().saturating_sub(4), y, 4, 1);
                frame.render_widget(
                    Paragraph::new(label)
                        .alignment(Alignment::Right)
                        .style(Style::default().fg(theme.accent).bg(background)),
                    action_area,
                );
                action_hits.push(ScmActionHit {
                    area: action_area,
                    action,
                });
                hits.push(SidebarHitArea {
                    area: Rect::new(list_area.x, y, list_area.width, 1),
                    target: SidebarHit::ChangeSection(*section),
                });
            }
            ChangeRow::Change {
                index,
                cursor,
                section: _,
            } => {
                let Some(change) = app.status.changes.get(*index) else {
                    continue;
                };
                let selected =
                    app.selected_change_section.is_none() && *cursor == app.change_cursor;
                let row_style = if selected {
                    Style::default().bg(theme.selected)
                } else {
                    Style::default().bg(theme.panel)
                };
                let path = change.parent_path();
                let available = list_area.width.saturating_sub(11) as usize;
                let name = truncate_middle(
                    &change.file_name(),
                    available.saturating_sub(path.width() + 1),
                );
                let line = Line::from(vec![
                    Span::styled(
                        if selected { " • " } else { "   " },
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(
                        name,
                        Style::default().fg(theme.text).add_modifier(if selected {
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
                        Style::default().fg(theme.muted),
                    ),
                ]);
                frame.render_widget(
                    Paragraph::new(line).style(row_style),
                    Rect::new(list_area.x, y, list_area.width.saturating_sub(7), 1),
                );
                let (action_label, action) = match change.area {
                    ChangeArea::Staged => ("[−]", ScmAction::Unstage(*index)),
                    ChangeArea::Conflict => ("[!]", ScmAction::Resolve(*index)),
                    ChangeArea::Unstaged => ("[+]", ScmAction::Stage(*index)),
                };
                let background = if selected {
                    theme.selected
                } else {
                    theme.panel
                };
                let status_area = Rect::new(list_area.right().saturating_sub(7), y, 3, 1);
                frame.render_widget(
                    Paragraph::new(change.status.code())
                        .alignment(Alignment::Right)
                        .style(
                            Style::default()
                                .fg(status_color(change.status, theme))
                                .bg(background)
                                .add_modifier(Modifier::BOLD),
                        ),
                    status_area,
                );
                let action_area = Rect::new(list_area.right().saturating_sub(4), y, 4, 1);
                frame.render_widget(
                    Paragraph::new(action_label)
                        .alignment(Alignment::Right)
                        .style(
                            Style::default()
                                .fg(theme.accent)
                                .bg(background)
                                .add_modifier(Modifier::BOLD),
                        ),
                    action_area,
                );
                action_hits.push(ScmActionHit {
                    area: action_area,
                    action,
                });
                hits.push(SidebarHitArea {
                    area: Rect::new(list_area.x, y, list_area.width, 1),
                    target: SidebarHit::Change(*index),
                });
            }
        }
    }

    draw_scrollbar(frame, list_area, app.sidebar_offset, rows.len(), theme);

    if visible.is_empty() {
        let message = if app.status.changes.is_empty() {
            "\n  ✓ Working tree clean\n\n  No pending changes"
        } else {
            "\n  No changes match this filter"
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(Style::default().fg(if app.status.changes.is_empty() {
                    theme.success
                } else {
                    theme.muted
                }))
                .wrap(Wrap { trim: false }),
            list_area,
        );
    }

    let controls_y = list_area.bottom();
    if controls_height >= 1 {
        let row = Rect::new(inner.x, controls_y, inner.width, 1);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" [c] Commit", Style::default().fg(theme.accent)),
                Span::styled("   [S] Stashes", Style::default().fg(theme.modified)),
            ]))
            .style(Style::default().bg(theme.panel_alt)),
            row,
        );
        action_hits.push(ScmActionHit {
            area: Rect::new(row.x, row.y, 12.min(row.width), 1),
            action: ScmAction::Commit,
        });
        action_hits.push(ScmActionHit {
            area: Rect::new(
                row.x.saturating_add(12).min(row.right()),
                row.y,
                row.width.saturating_sub(12),
                1,
            ),
            action: ScmAction::Stashes,
        });
    }
    if controls_height >= 2 {
        let row = Rect::new(inner.x, controls_y + 1, inner.width, 1);
        frame.render_widget(
            Paragraph::new(" [a] Stage All")
                .style(Style::default().fg(theme.added).bg(theme.panel_alt)),
            row,
        );
        action_hits.push(ScmActionHit {
            area: row,
            action: ScmAction::StageAll,
        });
    }
    if controls_height >= 3 {
        let row = Rect::new(inner.x, controls_y + 2, inner.width, 1);
        frame.render_widget(
            Paragraph::new(" [U] Unstage All   [d] Compare Branch")
                .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
            row,
        );
        action_hits.push(ScmActionHit {
            area: Rect::new(row.x, row.y, 17.min(row.width), 1),
            action: ScmAction::UnstageAll,
        });
        action_hits.push(ScmActionHit {
            area: Rect::new(
                row.x.saturating_add(17).min(row.right()),
                row.y,
                row.width.saturating_sub(17),
                1,
            ),
            action: ScmAction::CompareBranch,
        });
    }
    (hits, action_hits)
}

fn draw_history_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> Vec<SidebarHitArea> {
    let title = if app.filter.is_empty() {
        format!(
            " History · {}  {}{}  [b branch] ",
            app.history_branch_label(),
            app.history.len(),
            if app.history_complete { "" } else { "+" }
        )
    } else {
        format!(
            " History · {}  /{} ",
            app.history_branch_label(),
            app.filter
        )
    };
    let block = panel_block(
        title,
        app.focus == Focus::Sidebar && app.modal.is_none(),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return Vec::new();
    }

    let visible = app.visible_commit_indices();
    let height = inner.height as usize;
    ensure_offset(
        &mut app.sidebar_offset,
        app.history_cursor,
        height,
        visible.len(),
    );
    let mut hits = Vec::new();
    let end = (app.sidebar_offset + height).min(visible.len());
    for (row_offset, index) in visible
        .iter()
        .take(end)
        .skip(app.sidebar_offset)
        .enumerate()
    {
        let cursor = app.sidebar_offset + row_offset;
        let Some(commit) = app.history.get(*index) else {
            continue;
        };
        let selected = cursor == app.history_cursor;
        let y = inner.y + cells(row_offset);
        let row_style = Style::default().bg(if selected {
            theme.selected
        } else {
            theme.panel
        });
        let graph = history_glyph(commit, cursor);
        let badge = commit
            .decorations
            .first()
            .map(|decoration| format!("  {}", clean_decoration(decoration)))
            .unwrap_or_default();
        let reserved = commit.short_id.width() + 8;
        let subject = truncate_middle(
            &commit.subject,
            (inner.width as usize).saturating_sub(reserved + badge.width()),
        );
        let line = Line::from(vec![
            Span::styled(
                if selected { " › " } else { "   " },
                Style::default().fg(theme.accent),
            ),
            Span::styled(graph, Style::default().fg(graph_color(cursor, theme))),
            Span::styled(
                subject,
                Style::default().fg(theme.text).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Span::styled(badge, Style::default().fg(theme.modified)),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(row_style),
            Rect::new(inner.x, y, inner.width.saturating_sub(10), 1),
        );
        let sha_area = Rect::new(
            inner
                .right()
                .saturating_sub(1)
                .saturating_sub(cells(commit.short_id.width())),
            y,
            cells(commit.short_id.width()),
            1,
        );
        frame.render_widget(
            Paragraph::new(Line::from(interactive_span(
                commit.short_id.clone(),
                app.commit_open_target(&commit.id),
                sha_area,
                theme,
                link_hits,
            )))
            .alignment(Alignment::Right)
            .style(Style::default().bg(if selected {
                theme.selected
            } else {
                theme.panel
            })),
            Rect::new(inner.right().saturating_sub(10), y, 9, 1),
        );
        hits.push(SidebarHitArea {
            area: Rect::new(inner.x, y, inner.width, 1),
            target: SidebarHit::Commit(*index),
        });
    }

    draw_scrollbar(frame, inner, app.sidebar_offset, visible.len(), theme);

    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.history_loading {
                "\n  Loading commit history…"
            } else if app.history.is_empty() {
                "\n  No commits yet"
            } else {
                "\n  No commits match this filter"
            })
            .style(Style::default().fg(theme.muted)),
            inner,
        );
    } else if app.history_loading && inner.height > 1 {
        let area = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
        frame.render_widget(
            Paragraph::new("  Loading more commits…")
                .style(Style::default().fg(theme.accent).bg(theme.panel_alt)),
            area,
        );
    }
    hits
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
fn draw_pull_requests_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<SidebarHitArea> {
    let warning = if app.pull_request_warnings.is_empty() {
        String::new()
    } else {
        format!("  ⚠{}", app.pull_request_warnings.len())
    };
    let loading = app
        .pull_request_progress
        .map_or_else(String::new, |progress| {
            format!("  · {}%", progress.percent())
        });
    let cache = if app.pull_request_refreshing() {
        "  ⟳"
    } else if app.pull_request_served_from_cache() {
        "  · cached"
    } else {
        ""
    };
    let title = app.selected_pull_request().map_or_else(
        || format!(" Open Pull Request · on demand{loading}{warning} "),
        |pull_request| {
            let state = if pull_request.is_draft {
                "DRAFT"
            } else {
                pull_request.state.as_str()
            };
            format!(
                " Pull Request #{} · {state}{loading}{cache}{warning} ",
                pull_request.number
            )
        },
    );
    let block = panel_block(
        title,
        app.focus == Focus::Sidebar && app.modal.is_none(),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return Vec::new();
    }

    let controls_height = inner.height.min(3);
    let body_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(controls_height),
    );
    let mut hits = Vec::new();
    if app.pull_request.is_some() && body_area.height > 0 {
        let overview_width = body_area.width.saturating_mul(3) / 5;
        let overview_tab = Rect::new(body_area.x, body_area.y, overview_width, 1);
        let files_tab = Rect::new(
            overview_tab.right(),
            body_area.y,
            body_area.width.saturating_sub(overview_width),
            1,
        );
        let overview_label = format!(
            "PR {}",
            if app.pull_request_refreshing() {
                "⟳"
            } else {
                "↻"
            }
        );
        draw_pull_request_section_tab(
            frame,
            overview_tab,
            overview_label.clone(),
            app.pull_request_section == PullRequestSection::Overview,
            theme,
        );
        draw_pull_request_section_tab(
            frame,
            files_tab,
            format!("Files {}", app.pull_request_total_files),
            app.pull_request_section == PullRequestSection::Files,
            theme,
        );
        let overview_label_x = overview_tab.x
            + overview_tab
                .width
                .saturating_sub(cells(overview_label.width()))
                / 2;
        hits.push(SidebarHitArea {
            area: Rect::new(overview_label_x.saturating_add(3), overview_tab.y, 1, 1),
            target: SidebarHit::PullRequestRefresh,
        });
        hits.push(SidebarHitArea {
            area: overview_tab,
            target: SidebarHit::PullRequestOverview,
        });
        hits.push(SidebarHitArea {
            area: files_tab,
            target: SidebarHit::PullRequestFiles,
        });

        let list_area = Rect::new(
            body_area.x,
            body_area.y + 1,
            body_area.width,
            body_area.height.saturating_sub(1),
        );
        match app.pull_request_section {
            PullRequestSection::Files => {
                hits.extend(draw_pull_request_file_tree(frame, list_area, app, theme));
            }
            PullRequestSection::Overview => {
                hits.extend(draw_pull_request_check_list(frame, list_area, app, theme));
            }
        }
    } else if app.pull_request_loading {
        let skeleton_count = body_area.height.min(6);
        for offset in 0..skeleton_count {
            let y = body_area.y + offset;
            let width = body_area.width.saturating_sub(8 + (offset % 3) * 5);
            frame.render_widget(
                Paragraph::new(format!(
                    "   ◌ {}",
                    "─".repeat(width.saturating_sub(6) as usize)
                ))
                .style(Style::default().fg(theme.border).bg(theme.panel)),
                Rect::new(body_area.x, y, body_area.width, 1),
            );
        }
    }

    let controls_y = body_area.bottom();
    let repository_name = app.pull_request_repository.as_ref().map_or_else(
        || "auto-detect from remotes".to_owned(),
        GitHubRepository::display_name,
    );
    if controls_height >= 1 {
        let repository_area = Rect::new(inner.x, controls_y, inner.width, 1);
        frame.render_widget(
            Paragraph::new(truncate_middle(
                &format!(" repo {repository_name}  [o choose]"),
                inner.width as usize,
            ))
            .style(Style::default().fg(theme.text).bg(theme.panel_alt)),
            repository_area,
        );
        hits.push(SidebarHitArea {
            area: repository_area,
            target: SidebarHit::PullRequestChooseRepository,
        });
    }
    if controls_height >= 2 {
        let lookup_area = Rect::new(inner.x, controls_y + 1, inner.width, 1);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" PR # ", Style::default().fg(theme.accent)),
                Span::styled(
                    app.pull_request_lookup.value.as_str(),
                    Style::default().fg(theme.text),
                ),
                Span::styled(
                    if app.pull_request_lookup_active {
                        "  Enter lookup"
                    } else {
                        "  [/ lookup]"
                    },
                    Style::default().fg(theme.muted),
                ),
            ]))
            .style(Style::default().bg(if app.pull_request_lookup_active {
                theme.selected
            } else {
                theme.panel_alt
            })),
            lookup_area,
        );
        if app.pull_request_lookup_active {
            set_text_cursor(
                frame,
                Rect::new(
                    lookup_area.x + 6,
                    lookup_area.y,
                    lookup_area.width.saturating_sub(6),
                    1,
                ),
                &app.pull_request_lookup,
                false,
            );
        }
        hits.push(SidebarHitArea {
            area: lookup_area,
            target: SidebarHit::PullRequestLookup,
        });
    }
    if controls_height >= 3 {
        let status = if let Some(progress) = app.pull_request_progress {
            format!("{}% {}", progress.percent(), progress.label())
        } else {
            match app.pull_request_section {
                PullRequestSection::Files => {
                    let suffix = if app.pull_request_files_truncated {
                        " · list bounded"
                    } else {
                        ""
                    };
                    format!(
                        "j/k select · ←/→ folders · {} files{suffix}",
                        app.pull_request_total_files
                    )
                }
                PullRequestSection::Overview => {
                    if app.pull_request_checks_error.is_some() {
                        "checks unavailable · r retry".to_owned()
                    } else if app.pull_request_check_cursor.is_some() {
                        "[ / ] step · space fold · e all".to_owned()
                    } else {
                        format!("live · {}", app.live_refresh_label())
                    }
                }
            }
        };
        frame.render_widget(
            Paragraph::new(format!(" {status}"))
                .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
            Rect::new(inner.x, controls_y + 2, inner.width, 1),
        );
    }
    hits
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "the caller has no use for the value afterwards"
)]
fn draw_pull_request_section_tab(
    frame: &mut Frame<'_>,
    area: Rect,
    label: String,
    selected: bool,
    theme: &Theme,
) {
    frame.render_widget(
        Paragraph::new(truncate_end(&label, area.width as usize))
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.selected
                    } else {
                        theme.panel_alt
                    })
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        area,
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
fn draw_pull_request_file_tree(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<SidebarHitArea> {
    if app.pull_request_files.is_empty() {
        let message = if app.document_loading || app.pull_request_progress.is_some() {
            "\n  Preparing local diff index…"
        } else {
            "\n  No changed files"
        };
        frame.render_widget(
            Paragraph::new(message).style(Style::default().fg(theme.muted)),
            area,
        );
        return Vec::new();
    }
    let row_count = app.pull_request_tree_entries().len();
    app.pull_request_tree_cursor = app
        .pull_request_tree_cursor
        .min(row_count.saturating_sub(1));
    ensure_offset(
        &mut app.sidebar_offset,
        app.pull_request_tree_cursor,
        area.height as usize,
        row_count,
    );
    let rows = &app.pull_request_tree;
    let mut hits = Vec::new();
    for (offset, row) in rows
        .iter()
        .skip(app.sidebar_offset)
        .take(area.height as usize)
        .enumerate()
    {
        let row_index = app.sidebar_offset + offset;
        let y = area.y + cells(offset);
        let selected = row_index == app.pull_request_tree_cursor;
        let background = if selected {
            theme.selected
        } else {
            theme.panel
        };
        match row {
            PullRequestTreeEntry::Directory { path, label, depth } => {
                let background = if selected {
                    theme.selected
                } else {
                    theme.panel_alt
                };
                let indent_width = depth.saturating_mul(2).min(16);
                let available = (area.width as usize)
                    .saturating_sub(indent_width)
                    .saturating_sub(5);
                let icon = if app.pull_request_directory_collapsed(path) {
                    "›"
                } else {
                    "⌄"
                };
                frame.render_widget(
                    Paragraph::new(format!(
                        " {}{icon} {}/",
                        "  ".repeat((*depth).min(8)),
                        truncate_end(label, available),
                    ))
                    .style(
                        Style::default()
                            .fg(if selected { theme.text } else { theme.muted })
                            .bg(background)
                            .add_modifier(if selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                    Rect::new(area.x, y, area.width, 1),
                );
                hits.push(SidebarHitArea {
                    area: Rect::new(area.x, y, area.width, 1),
                    target: SidebarHit::PullRequestDirectory(path.clone()),
                });
            }
            PullRequestTreeEntry::File { depth, index } => {
                let Some(file) = app.pull_request_files.get(*index) else {
                    continue;
                };
                let indent_width = depth.saturating_mul(2).min(16);
                let name = file.path.file_name().map_or_else(
                    || file.path.to_string_lossy(),
                    |name| name.to_string_lossy(),
                );
                let available = (area.width as usize)
                    .saturating_sub(indent_width)
                    .saturating_sub(7);
                let line = format!(
                    " {}{}{}",
                    "  ".repeat((*depth).min(8)),
                    if selected { "• " } else { "  " },
                    truncate_end(&name, available),
                );
                frame.render_widget(
                    Paragraph::new(line).style(
                        Style::default()
                            .fg(theme.text)
                            .bg(background)
                            .add_modifier(if selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                    Rect::new(area.x, y, area.width.saturating_sub(3), 1),
                );
                frame.render_widget(
                    Paragraph::new(pull_request_file_status_code(file.status))
                        .alignment(Alignment::Right)
                        .style(
                            Style::default()
                                .fg(pull_request_file_status_color(file.status, theme))
                                .bg(background)
                                .add_modifier(Modifier::BOLD),
                        ),
                    Rect::new(area.right().saturating_sub(3), y, 2, 1),
                );
                hits.push(SidebarHitArea {
                    area: Rect::new(area.x, y, area.width, 1),
                    target: SidebarHit::PullRequestFile(*index),
                });
            }
        }
    }
    draw_scrollbar(frame, area, app.sidebar_offset, rows.len(), theme);
    hits
}

const fn pull_request_file_status_code(status: PullRequestFileStatus) -> &'static str {
    match status {
        PullRequestFileStatus::Added => "A",
        PullRequestFileStatus::Modified => "M",
        PullRequestFileStatus::Deleted => "D",
        PullRequestFileStatus::Renamed => "R",
        PullRequestFileStatus::Copied => "C",
        PullRequestFileStatus::TypeChanged => "T",
        PullRequestFileStatus::Unmerged => "U",
        PullRequestFileStatus::Unknown => "?",
    }
}

const fn pull_request_file_status_color(status: PullRequestFileStatus, theme: &Theme) -> Color {
    match status {
        PullRequestFileStatus::Added => theme.added,
        PullRequestFileStatus::Deleted => theme.removed,
        PullRequestFileStatus::Renamed
        | PullRequestFileStatus::Copied
        | PullRequestFileStatus::Modified
        | PullRequestFileStatus::TypeChanged => theme.modified,
        PullRequestFileStatus::Unmerged => theme.conflict,
        PullRequestFileStatus::Unknown => theme.muted,
    }
}

/// The overview sidebar is the pull request itself on row zero followed by its
/// checks, so one list carries both the way back to the conversation and the way
/// into any run's log.
fn draw_pull_request_check_list(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<SidebarHitArea> {
    if area.height == 0 {
        return Vec::new();
    }
    let rows = app.pull_request_checks.len() + 1;
    let cursor_row = app
        .pull_request_check_cursor
        .map_or(0, |cursor| cursor.saturating_add(1));
    ensure_offset(
        &mut app.sidebar_offset,
        cursor_row,
        area.height as usize,
        rows,
    );

    let mut hits = Vec::new();
    for (offset, row) in (app.sidebar_offset..rows)
        .take(area.height as usize)
        .enumerate()
    {
        let y = area.y + cells(offset);
        let row_area = Rect::new(area.x, y, area.width, 1);
        let selected = row == cursor_row;
        let background = if selected {
            theme.selected
        } else {
            theme.panel
        };
        let marker = Span::styled(
            if selected { " › " } else { "   " },
            Style::default().fg(theme.accent),
        );
        let (line, target) = if row == 0 {
            (
                Line::from(vec![
                    marker,
                    Span::styled(
                        "Conversation",
                        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        conversation_row_suffix(app),
                        Style::default().fg(theme.muted),
                    ),
                ]),
                SidebarHit::PullRequestConversation,
            )
        } else {
            let index = row - 1;
            let Some(check) = app.pull_request_checks.get(index) else {
                continue;
            };
            let (icon, color) = pull_request_check_icon(check.status, theme);
            let workflow = if check.workflow.is_empty() {
                String::new()
            } else {
                format!("  {}", check.workflow)
            };
            let reserved = 6 + workflow.width();
            (
                Line::from(vec![
                    marker,
                    Span::styled(format!("{icon} "), Style::default().fg(color)),
                    Span::styled(
                        truncate_end(&check.name, (area.width as usize).saturating_sub(reserved)),
                        Style::default().fg(theme.text),
                    ),
                    Span::styled(workflow, Style::default().fg(theme.muted)),
                ]),
                SidebarHit::PullRequestCheck(index),
            )
        };
        if row == 0 {
            let refresh_x = area.x.saturating_add(cells(line.width().saturating_sub(1)));
            if refresh_x < area.right() {
                hits.push(SidebarHitArea {
                    area: Rect::new(refresh_x, y, 1, 1),
                    target: SidebarHit::PullRequestConversationRefresh,
                });
            }
        }
        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(background)),
            row_area,
        );
        hits.push(SidebarHitArea {
            area: row_area,
            target,
        });
    }
    draw_scrollbar(frame, area, app.sidebar_offset, rows, theme);

    if app.pull_request_checks.is_empty() && area.height > 1 {
        let message = if app.pull_request_checks_loading {
            "  Loading checks…".to_owned()
        } else if let Some(error) = app.pull_request_checks_error.as_deref() {
            format!("  {error}")
        } else {
            "  No checks reported".to_owned()
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(Style::default().fg(theme.muted).bg(theme.panel))
                .wrap(Wrap { trim: true }),
            Rect::new(area.x, area.y + 1, area.width, area.height - 1),
        );
    }
    hits
}

fn conversation_row_suffix(app: &App) -> String {
    let refresh = if app.pull_request_conversation_loading {
        "  ⟳"
    } else {
        "  ↻"
    };
    if app.pull_request_conversation_error.is_some() {
        return format!("  ⚠{refresh}");
    }
    let comments = app.pull_request_conversation.comment_count();
    if comments == 0 {
        refresh.to_owned()
    } else {
        format!("  {comments}{refresh}")
    }
}

const fn pull_request_check_icon(
    status: PullRequestCheckStatus,
    theme: &Theme,
) -> (&'static str, Color) {
    match status {
        PullRequestCheckStatus::Passed => ("✓", theme.success),
        PullRequestCheckStatus::Failed => ("×", theme.error),
        PullRequestCheckStatus::Pending => ("◌", theme.accent),
        PullRequestCheckStatus::Skipped => ("−", theme.muted),
        PullRequestCheckStatus::Cancelled => ("■", theme.removed),
        PullRequestCheckStatus::Unknown => ("?", theme.muted),
    }
}

/// A pre-wrapped content row, optionally anchored to a check step so a click or
/// the step cursor can find it after scrolling.
type ContentRow = PullRequestContentRow;

impl PullRequestContentRow {
    const fn plain(line: Line<'static>) -> Self {
        Self {
            line,
            step: None,
            wide: false,
        }
    }

    const fn wide(line: Line<'static>) -> Self {
        Self {
            line,
            step: None,
            wide: true,
        }
    }

    fn blank() -> Self {
        Self::plain(Line::default())
    }

    fn text(value: impl Into<String>, style: Style) -> Self {
        Self::plain(Line::from(Span::styled(value.into(), style)))
    }
}

fn draw_pull_request_overview(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> Vec<ContentStepHit> {
    let showing_check = app.pull_request_check_cursor.is_some();
    let focused = app.focus == Focus::Content && app.modal.is_none();
    let inner = panel_block(String::new(), focused, theme).inner(area);
    if inner.width == 0 || inner.height == 0 {
        frame.render_widget(
            panel_block(overview_title(app, showing_check), focused, theme),
            area,
        );
        return Vec::new();
    }

    let width = inner.width as usize;
    let rows_key = (
        showing_check,
        width,
        app.pull_request_generation,
        app.pull_request_checks_generation,
        app.pull_request_conversation_generation,
        app.pull_request_check_log_generation,
        app.pull_request_content_generation,
    );
    if app.pull_request_content_rows_key != Some(rows_key) {
        app.pull_request_content_rows = if showing_check {
            check_run_rows(app, width, theme)
        } else {
            conversation_rows(app, width, theme)
        };
        app.pull_request_content_rows_key = Some(rows_key);
    }
    let rows = &app.pull_request_content_rows;
    let content_width = rows
        .iter()
        .filter(|row| row.wide)
        .map(|row| row.line.width())
        .max()
        .unwrap_or_default();
    let overflow = content_width.saturating_sub(width);
    app.horizontal_scroll = app.horizontal_scroll.min(overflow);

    let mut title = overview_title(app, showing_check);
    if overflow > 0 {
        title.push_str("·  ←/→ ");
        title.push_str(&app.horizontal_scroll.to_string());
        title.push('/');
        title.push_str(&overflow.to_string());
        title.push(' ');
    }
    frame.render_widget(panel_block(title, focused, theme), area);

    if showing_check && app.pull_request_step_reveal {
        app.pull_request_step_reveal = false;
        if let Some(cursor_row) = rows
            .iter()
            .position(|row| row.step == Some(app.pull_request_step_cursor))
        {
            ensure_offset(
                &mut app.content_scroll,
                cursor_row,
                inner.height as usize,
                rows.len(),
            );
        }
    }
    let max_scroll = rows.len().saturating_sub(inner.height as usize);
    app.content_scroll = app.content_scroll.min(max_scroll);
    app.content_at_bottom = app.content_scroll == max_scroll;

    let mut hits = Vec::new();
    for (offset, row) in rows
        .iter()
        .skip(app.content_scroll)
        .take(inner.height as usize)
        .enumerate()
    {
        let row_area = Rect::new(inner.x, inner.y + cells(offset), inner.width, 1);
        let selected = showing_check && row.step == Some(app.pull_request_step_cursor);
        frame.render_widget(
            Paragraph::new(if row.wide {
                shift_line(&row.line, app.horizontal_scroll, width)
            } else {
                row.line.clone()
            })
            .style(Style::default().bg(if selected {
                theme.selected
            } else {
                theme.panel
            })),
            row_area,
        );
        if let Some(step) = row.step {
            hits.push(ContentStepHit {
                area: row_area,
                step,
            });
        } else if app.content_scroll.saturating_add(offset) == 0
            && let Some(pull_request) = app.selected_pull_request()
            && let Some(target) = app.pull_request_open_target(pull_request.number)
        {
            let label = format!("#{}", pull_request.number);
            let area = clipped_link_area(row_area.x, row_area.y, label.width(), row_area);
            if area.width > 0 {
                link_hits.push(LinkHit { area, target });
            }
        }
    }
    draw_scrollbar(frame, inner, app.content_scroll, rows.len(), theme);
    hits
}

fn overview_title(app: &App, showing_check: bool) -> String {
    let Some(pull_request) = app.selected_pull_request() else {
        return " Pull Request ".to_owned();
    };
    if showing_check {
        let name = app
            .selected_pull_request_check()
            .map_or("Check", |check| check.name.as_str());
        let loading = if app.pull_request_check_log_loading {
            "  ⟳"
        } else {
            ""
        };
        return format!(" PR #{} · {name}{loading} ", pull_request.number);
    }
    let state = if pull_request.is_draft {
        "DRAFT"
    } else {
        pull_request.state.as_str()
    };
    let loading = if app.pull_request_conversation_loading {
        "  ⟳"
    } else if app.pull_request_served_from_cache() {
        "  · cached"
    } else {
        ""
    };
    format!(" PR #{} · {state}{loading} ", pull_request.number)
}

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
fn conversation_rows(app: &App, width: usize, theme: &Theme) -> Vec<ContentRow> {
    let mut rows = Vec::new();
    let Some(pull_request) = app.selected_pull_request() else {
        if let Some(error) = app.pull_request_error.as_deref() {
            rows.push(ContentRow::text(
                "  This pull request could not be opened",
                Style::default()
                    .fg(theme.error)
                    .add_modifier(Modifier::BOLD),
            ));
            rows.push(ContentRow::blank());
            for (_, text) in wrap_prose(error, width.saturating_sub(2)) {
                rows.push(ContentRow::text(
                    format!("  {text}"),
                    Style::default().fg(theme.error),
                ));
            }
            rows.push(ContentRow::blank());
            rows.push(ContentRow::text(
                "  Press r to try again, or o to choose another repository",
                Style::default().fg(theme.muted),
            ));
            return rows;
        }
        rows.push(ContentRow::text(
            if app.pull_request_loading {
                "  Fetching pull-request metadata…"
            } else {
                "  Enter a pull-request number to open one"
            },
            Style::default().fg(theme.muted),
        ));
        return rows;
    };

    let state = if pull_request.is_draft {
        "DRAFT"
    } else {
        pull_request.state.as_str()
    };
    rows.push(ContentRow::wide(Line::from(vec![
        Span::styled(
            format!("#{}  ", pull_request.number),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        Span::styled(
            pull_request.title.clone(),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
    ])));
    rows.push(ContentRow::wide(detail_line(
        "State",
        format!(
            "{state}  ·  @{}  ·  opened {}  ·  updated {}",
            pull_request.author,
            format_local_timestamp(&pull_request.created_at),
            format_local_timestamp(&pull_request.updated_at)
        ),
        theme,
    )));
    rows.push(ContentRow::wide(detail_line(
        "Source",
        format!(
            "{}{}",
            pull_request.head_label(),
            if pull_request.is_cross_repository {
                "  ·  fork"
            } else {
                ""
            }
        ),
        theme,
    )));
    rows.push(ContentRow::wide(detail_line(
        "Destination",
        pull_request.base_label(),
        theme,
    )));
    rows.push(ContentRow::wide(Line::from(vec![
        Span::styled(
            format!("{:<DETAIL_LABEL_WIDTH$}", "Changes"),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            format!(
                "{} file{}  ",
                pull_request.changed_files,
                if pull_request.changed_files == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            Style::default().fg(theme.text),
        ),
        Span::styled(
            format!("+{}", pull_request.additions),
            Style::default()
                .fg(theme.added)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("-{}", pull_request.deletions),
            Style::default()
                .fg(theme.removed)
                .add_modifier(Modifier::BOLD),
        ),
    ])));
    rows.push(ContentRow::wide(check_summary_line(app, theme)));
    rows.push(ContentRow::wide(detail_line(
        "URL",
        pull_request.url.clone(),
        theme,
    )));

    rows.push(ContentRow::blank());
    rows.push(ContentRow::plain(section_rule("Description", width, theme)));
    if pull_request.description.trim().is_empty() {
        rows.push(ContentRow::text(
            "  No description provided",
            Style::default().fg(theme.muted),
        ));
    } else {
        push_prose(&mut rows, &pull_request.description, " ", width, theme);
    }

    rows.push(ContentRow::blank());
    rows.push(ContentRow::plain(section_rule(
        "Conversation",
        width,
        theme,
    )));
    if let Some(error) = app.pull_request_conversation_error.as_deref() {
        rows.push(ContentRow::text(
            format!("  {error}"),
            Style::default().fg(theme.error),
        ));
        return rows;
    }
    if app.pull_request_conversation.entries.is_empty() {
        rows.push(ContentRow::text(
            if app.pull_request_conversation_loading {
                "  Loading the conversation…"
            } else {
                "  No activity yet"
            },
            Style::default().fg(theme.muted),
        ));
        return rows;
    }
    if app.pull_request_conversation.truncated {
        rows.push(ContentRow::text(
            "  Older activity was omitted to keep this view bounded",
            Style::default().fg(theme.muted),
        ));
    }
    for entry in &app.pull_request_conversation.entries {
        rows.push(ContentRow::blank());
        push_conversation_entry(&mut rows, entry, width, theme);
    }
    rows
}

fn push_conversation_entry(
    rows: &mut Vec<ContentRow>,
    entry: &ConversationEntry,
    width: usize,
    theme: &Theme,
) {
    let (icon, color, action) = conversation_marker(entry, theme);
    let stamp = format_local_timestamp(&entry.timestamp);
    let stamp = if stamp.is_empty() {
        String::new()
    } else {
        format!("  ·  {stamp}")
    };
    let reserved = 3 + entry.actor.width() + stamp.width();
    let action = truncate_end(&action, width.saturating_sub(reserved));
    rows.push(ContentRow::plain(Line::from(vec![
        Span::styled(format!("{icon} "), Style::default().fg(color)),
        Span::styled(
            format!("@{}", entry.actor),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {action}"), Style::default().fg(theme.muted)),
        Span::styled(stamp, Style::default().fg(theme.muted)),
    ])));

    if !entry.context.is_empty() {
        for line in entry.context.lines().take(8) {
            let style = match line.as_bytes().first() {
                Some(b'+') => Style::default().fg(theme.added),
                Some(b'-') => Style::default().fg(theme.removed),
                _ => Style::default().fg(theme.muted),
            };
            rows.push(ContentRow::wide(Line::from(vec![
                Span::styled(" │ ▏ ", Style::default().fg(theme.border)),
                Span::styled(line.to_owned(), style),
            ])));
        }
    }
    if entry.kind != ConversationKind::Opened
        && entry.kind.has_body()
        && !entry.body.trim().is_empty()
    {
        push_prose(rows, &entry.body, " │", width, theme);
    }
}

fn conversation_marker(entry: &ConversationEntry, theme: &Theme) -> (&'static str, Color, String) {
    match entry.kind {
        ConversationKind::Opened => (
            "◆",
            theme.accent,
            format!("opened this pull request from {}", entry.detail),
        ),
        ConversationKind::Comment => ("▣", theme.text, "commented".to_owned()),
        ConversationKind::Review => {
            let (icon, color) = match entry.detail.to_ascii_lowercase().as_str() {
                "approved" => ("✓", theme.success),
                "changes_requested" => ("×", theme.error),
                _ => ("▣", theme.accent),
            };
            (
                icon,
                color,
                format!("reviewed · {}", entry.detail.to_lowercase()),
            )
        }
        ConversationKind::ReviewComment => (
            "▸",
            theme.modified,
            format!("commented on {}", entry.detail),
        ),
        ConversationKind::Commit => ("●", theme.muted, format!("pushed {}", entry.detail)),
        ConversationKind::ForcePush => (
            "↻",
            theme.modified,
            format!(
                "force-pushed{}",
                if entry.reference.is_empty() {
                    String::new()
                } else {
                    format!(" to {}", short_oid(&entry.reference))
                }
            ),
        ),
        ConversationKind::Merged => ("⏵", theme.success, "merged this pull request".to_owned()),
        ConversationKind::Closed => ("×", theme.removed, "closed this pull request".to_owned()),
        ConversationKind::Reopened => ("◆", theme.accent, "reopened this pull request".to_owned()),
        ConversationKind::Labeled => (
            "◈",
            theme.muted,
            format!("added the {} label", entry.detail),
        ),
        ConversationKind::Unlabeled => (
            "◈",
            theme.muted,
            format!("removed the {} label", entry.detail),
        ),
        ConversationKind::Renamed => (
            "✎",
            theme.muted,
            format!("renamed this from {}", entry.detail),
        ),
        ConversationKind::ReadyForReview => {
            ("◆", theme.accent, "marked this ready for review".to_owned())
        }
        ConversationKind::ConvertedToDraft => {
            ("◇", theme.muted, "converted this to a draft".to_owned())
        }
        ConversationKind::ReviewRequested => (
            "◎",
            theme.muted,
            format!("requested a review from {}", entry.detail),
        ),
        ConversationKind::ReviewRequestRemoved => (
            "◎",
            theme.muted,
            format!("removed the review request for {}", entry.detail),
        ),
        ConversationKind::Assigned => ("◎", theme.muted, format!("assigned {}", entry.detail)),
        ConversationKind::Unassigned => ("◎", theme.muted, format!("unassigned {}", entry.detail)),
        ConversationKind::CrossReferenced => (
            "⇥",
            theme.muted,
            format!("referenced this from #{}", entry.detail),
        ),
        ConversationKind::HeadRefDeleted => {
            ("⌫", theme.muted, "deleted the source branch".to_owned())
        }
        ConversationKind::HeadRefRestored => {
            ("↺", theme.muted, "restored the source branch".to_owned())
        }
        ConversationKind::BaseRefChanged => (
            "⇄",
            theme.modified,
            "changed the destination branch".to_owned(),
        ),
        ConversationKind::Other => ("·", theme.muted, "updated this pull request".to_owned()),
    }
}

fn check_summary_line(app: &App, theme: &Theme) -> Line<'static> {
    let count = |status: PullRequestCheckStatus| {
        app.pull_request_checks
            .iter()
            .filter(|check| check.status == status)
            .count()
    };
    let passed = count(PullRequestCheckStatus::Passed);
    let pending = count(PullRequestCheckStatus::Pending);
    let failed = count(PullRequestCheckStatus::Failed);
    let mut spans = vec![Span::styled(
        format!("{:<DETAIL_LABEL_WIDTH$}", "Checks"),
        Style::default().fg(theme.muted),
    )];
    if app.pull_request_checks.is_empty() {
        spans.push(Span::styled(
            if app.pull_request_checks_loading {
                "loading…"
            } else {
                "none reported"
            },
            Style::default().fg(theme.muted),
        ));
        return Line::from(spans);
    }
    spans.push(Span::styled(
        format!("✓{passed}  "),
        Style::default().fg(theme.success),
    ));
    spans.push(Span::styled(
        format!("◌{pending}  "),
        Style::default().fg(theme.accent),
    ));
    spans.push(Span::styled(
        format!("×{failed}"),
        Style::default().fg(theme.error),
    ));
    Line::from(spans)
}

fn check_run_rows(app: &App, width: usize, theme: &Theme) -> Vec<ContentRow> {
    let mut rows = Vec::new();
    let Some(check) = app.selected_pull_request_check() else {
        return rows;
    };
    let (icon, color) = pull_request_check_icon(check.status, theme);
    rows.push(ContentRow::wide(Line::from(vec![
        Span::styled(
            format!("{icon} "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            check.name.clone(),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
    ])));
    rows.push(ContentRow::wide(detail_line(
        "Workflow",
        format!("{}  ·  {}", check.workflow, check.state.to_lowercase()),
        theme,
    )));
    if !check.started_at.is_empty() {
        rows.push(ContentRow::wide(detail_line(
            "Ran",
            format!(
                "{}{}",
                format_local_timestamp(&check.started_at),
                match check.duration_label() {
                    duration if duration.is_empty() => String::new(),
                    duration => format!("  ·  {duration}"),
                }
            ),
            theme,
        )));
    }
    if !check.description.is_empty() {
        rows.push(ContentRow::wide(detail_line(
            "Details",
            check.description.clone(),
            theme,
        )));
    }
    if !check.link.is_empty() {
        rows.push(ContentRow::wide(detail_line(
            "URL",
            check.link.clone(),
            theme,
        )));
    }
    rows.push(ContentRow::blank());

    if let Some(error) = app.pull_request_check_log_error.as_deref() {
        rows.push(ContentRow::text(
            format!("  {error}"),
            Style::default().fg(theme.error),
        ));
        return rows;
    }
    let Some(log) = app.pull_request_check_log.as_ref() else {
        rows.push(ContentRow::text(
            if app.pull_request_check_log_loading {
                "  Loading the run log…"
            } else {
                "  No log loaded"
            },
            Style::default().fg(theme.muted),
        ));
        return rows;
    };
    if let Some(reason) = log.unavailable.as_deref() {
        rows.push(ContentRow::text(
            format!("  {reason}"),
            Style::default().fg(theme.muted),
        ));
        return rows;
    }
    rows.push(ContentRow::plain(section_rule(
        &format!("{} steps", log.steps.len()),
        width,
        theme,
    )));
    if log.log_pending {
        rows.push(ContentRow::text(
            "  Waiting for the runner to write its first output",
            Style::default().fg(theme.muted),
        ));
    }
    let now = crate::git::github::unix_now();
    for step in &log.steps {
        let expanded = app.check_step_expanded(step.number);
        rows.push(check_step_row(step, expanded, now, width, theme));
        if expanded {
            if step.lines.is_empty() && step.status.is_running() {
                rows.push(ContentRow::text(
                    "   │ waiting for output…",
                    Style::default().fg(theme.accent),
                ));
            } else {
                push_log_lines(&mut rows, &step.lines, theme);
            }
        }
    }
    if !log.loose_lines.is_empty() {
        rows.push(ContentRow::plain(section_rule(
            "Runner output",
            width,
            theme,
        )));
        push_log_lines(&mut rows, &log.loose_lines, theme);
    }
    if log.truncated {
        rows.push(ContentRow::text(
            "  … log truncated to keep Quinjet responsive …",
            Style::default().fg(theme.muted),
        ));
    }
    rows
}

/// A step reads as one row: fold state, outcome, name, and how long it took,
/// with the duration pushed to the right edge the way a run page shows it.
fn check_step_row(
    step: &CheckStep,
    expanded: bool,
    now: i64,
    width: usize,
    theme: &Theme,
) -> ContentRow {
    let (icon, color) = pull_request_check_icon(step.status, theme);
    let duration = step.duration_label(now);
    let reserved = 8 + duration.width();
    let name = truncate_end(&step.name, width.saturating_sub(reserved));
    let padding = width
        .saturating_sub(reserved)
        .saturating_sub(name.width())
        .saturating_add(1);
    ContentRow {
        line: Line::from(vec![
            Span::styled(
                if expanded { " ⌄ " } else { " › " },
                Style::default().fg(theme.muted),
            ),
            Span::styled(format!("{icon} "), Style::default().fg(color)),
            Span::styled(name, Style::default().fg(theme.text)),
            Span::styled(" ".repeat(padding), Style::default()),
            Span::styled(duration, Style::default().fg(theme.muted)),
        ]),
        step: Some(step.number),
        wide: false,
    }
}

fn push_log_lines(rows: &mut Vec<ContentRow>, lines: &[CheckLogLine], theme: &Theme) {
    if lines.is_empty() {
        rows.push(ContentRow::text(
            "   │ no output",
            Style::default().fg(theme.muted),
        ));
        return;
    }
    for line in lines {
        rows.push(ContentRow::wide(Line::from(vec![
            Span::styled("   │ ", Style::default().fg(theme.border)),
            Span::styled(
                line.text.clone(),
                Style::default().fg(log_severity_color(line.severity, theme)),
            ),
        ])));
    }
}

const fn log_severity_color(severity: CheckLogSeverity, theme: &Theme) -> Color {
    match severity {
        CheckLogSeverity::Normal => theme.text,
        CheckLogSeverity::Command => theme.accent,
        CheckLogSeverity::Notice | CheckLogSeverity::Warning => theme.modified,
        CheckLogSeverity::Error => theme.error,
    }
}

fn section_rule(label: &str, width: usize, theme: &Theme) -> Line<'static> {
    let label = format!(" {label} ");
    let fill = width.saturating_sub(label.width()).saturating_sub(2);
    Line::from(vec![
        Span::styled("──", Style::default().fg(theme.border)),
        Span::styled(label, Style::default().fg(theme.muted)),
        Span::styled("─".repeat(fill), Style::default().fg(theme.border)),
    ])
}

fn short_oid(value: &str) -> String {
    value.chars().take(7).collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProseStyle {
    Text,
    Heading,
    Bullet,
    Code,
    Quote,
}

fn prose_style(style: ProseStyle, theme: &Theme) -> Style {
    match style {
        ProseStyle::Text | ProseStyle::Bullet => Style::default().fg(theme.text),
        ProseStyle::Heading => Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ProseStyle::Code => Style::default().fg(theme.modified),
        ProseStyle::Quote => Style::default().fg(theme.muted),
    }
}

/// Render a body under its header behind a continuous gutter, so a long comment
/// stays visibly attached to the person who wrote it. Code carries a second
/// marker because it is the one kind of line that is not wrapped to the pane.
fn push_prose(rows: &mut Vec<ContentRow>, body: &str, gutter: &str, width: usize, theme: &Theme) {
    let available = width.saturating_sub(gutter.width() + 2);
    for (style, text) in wrap_prose(body, available) {
        let code = style == ProseStyle::Code;
        let line = Line::from(vec![
            Span::styled(
                format!("{gutter}{}", if code { " ▏ " } else { "  " }),
                Style::default().fg(theme.border),
            ),
            Span::styled(text, prose_style(style, theme)),
        ]);
        rows.push(if code {
            ContentRow::wide(line)
        } else {
            ContentRow::plain(line)
        });
    }
}

/// Window a composed row horizontally, in display columns rather than bytes, so
/// wide code and log lines can be read past the edge of the pane.
fn shift_line(line: &Line<'static>, skip: usize, width: usize) -> Line<'static> {
    let mut spans = Vec::with_capacity(line.spans.len());
    let mut scanned = 0;
    let mut used = 0;
    for span in &line.spans {
        if used >= width {
            break;
        }
        let span_width = span.content.width();
        if scanned + span_width <= skip {
            scanned += span_width;
            continue;
        }
        let text = slice_width(&span.content, skip.saturating_sub(scanned), width - used);
        scanned += span_width;
        if text.is_empty() {
            continue;
        }
        used += text.width();
        spans.push(Span::styled(text, span.style));
    }
    Line::from(spans)
}

#[expect(
    clippy::option_if_let_else,
    reason = "the branch is one arm of a longer chain that map_or_else cannot express"
)]
/// Wrap a Markdown body to a fixed width, keeping paragraph breaks, list
/// structure and fenced code intact. Code is truncated rather than wrapped so
/// its own indentation still reads correctly.
fn wrap_prose(value: &str, width: usize) -> Vec<(ProseStyle, String)> {
    let width = width.max(8);
    let mut output = Vec::new();
    let mut fenced = false;
    let mut previous_blank = true;
    for raw_line in value.lines() {
        let trimmed = raw_line.trim_end();
        if trimmed.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            previous_blank = false;
            output.push((ProseStyle::Code, trimmed.to_owned()));
            continue;
        }
        if trimmed.trim().is_empty() {
            if !previous_blank {
                output.push((ProseStyle::Text, String::new()));
                previous_blank = true;
            }
            continue;
        }
        previous_blank = false;
        let content = trimmed.trim_start();
        let (style, indent, body) = if let Some(rest) = content.strip_prefix("> ") {
            (ProseStyle::Quote, "  ", rest)
        } else if content.starts_with('#') {
            (
                ProseStyle::Heading,
                "",
                content.trim_start_matches('#').trim_start(),
            )
        } else if let Some(rest) = ["- ", "* ", "+ "]
            .into_iter()
            .find_map(|marker| content.strip_prefix(marker))
        {
            (ProseStyle::Bullet, "  ", rest)
        } else {
            (ProseStyle::Text, "", content)
        };
        let prefix = if style == ProseStyle::Bullet {
            "• "
        } else {
            ""
        };
        let available = width.saturating_sub(indent.width() + prefix.width());
        let body = body.replace('*', "");
        for (index, wrapped) in wrap_words(&body, available).into_iter().enumerate() {
            let lead = if index == 0 {
                format!("{indent}{prefix}")
            } else {
                " ".repeat(indent.width() + prefix.width())
            };
            output.push((style, format!("{lead}{wrapped}")));
        }
    }
    while output.last().is_some_and(|(_, text)| text.is_empty()) {
        drop(output.pop());
    }
    output
}

fn wrap_words(value: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in value.split_whitespace() {
        let word_width = word.width();
        if current.is_empty() {
            current = if word_width > width {
                lines.push(truncate_end(word, width));
                continue;
            } else {
                word.to_owned()
            };
        } else if current.width() + 1 + word_width <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            if word_width > width {
                lines.push(truncate_end(word, width));
            } else {
                word.clone_into(&mut current);
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn draw_content(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> (Option<Rect>, Vec<ContentFileHit>, Vec<ContentStepHit>) {
    if app.view == View::PullRequests && app.pull_request_section == PullRequestSection::Overview {
        let step_hits = draw_pull_request_overview(frame, area, app, theme, link_hits);
        return (None, Vec::new(), step_hits);
    }
    let file_action = if app.preview_files_collapsible() {
        if app.preview_files_all_collapsed() {
            "  [e Expand all]"
        } else {
            "  [e Collapse all]"
        }
    } else {
        ""
    };
    let loading = app.pull_request_progress.map_or_else(
        || {
            if app.document_loading
                && !(app.view == View::PullRequests && app.document.file_count() > 0)
            {
                "  ⟳".to_owned()
            } else {
                String::new()
            }
        },
        |progress| format!("  ⟳ {}%", progress.percent()),
    );
    let title_width = (area.width as usize)
        .saturating_sub(loading.width())
        .saturating_sub(file_action.width())
        .saturating_sub(4);
    let title = format!(
        " {}{}{} ",
        truncate_middle(&app.document.title, title_width),
        loading,
        file_action,
    );
    let block = panel_block(
        title,
        app.focus == Focus::Content && app.modal.is_none(),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return (None, Vec::new(), Vec::new());
    }

    let details_rows = if app.document.commit_details.is_some() {
        commit_details_row_count(inner.height)
    } else if app.view != View::PullRequests && app.document.pull_request_details.is_some() {
        pull_request_details_row_count(inner.height)
    } else {
        0
    };
    let side_by_side = app.diff_layout == DiffLayout::SideBySide && inner.width >= 72;
    let side_rows = optional_side_by_side_rows(side_by_side, &app.document, app);
    let unified_rows = if side_by_side {
        None
    } else {
        Some(unified_row_indices(&app.document, app))
    };
    let diff_rows = side_rows
        .as_ref()
        .map_or_else(|| unified_rows.as_ref().map_or(0, Vec::len), Vec::len);
    let visual_length = details_rows + diff_rows;
    let max_scroll = visual_length.saturating_sub(inner.height as usize);
    app.content_scroll = app.content_scroll.min(max_scroll);

    let mut diff_area = inner;
    let mut diff_scroll = app.content_scroll;
    if diff_scroll < details_rows {
        let visible_details = details_rows - diff_scroll;
        let details_height = cells(visible_details.min(inner.height as usize));
        let details_area = Rect::new(inner.x, inner.y, inner.width, details_height);
        if app.document.commit_details.is_some() {
            draw_commit_details_scrolled(
                frame,
                details_area,
                app,
                diff_scroll,
                details_rows,
                theme,
                link_hits,
            );
        } else if let Some(details) = app.document.pull_request_details.as_ref() {
            draw_pull_request_details_scrolled(
                frame,
                details_area,
                details,
                diff_scroll,
                details_rows,
                theme,
            );
        }
        diff_area = Rect::new(
            inner.x,
            inner.y.saturating_add(details_height),
            inner.width,
            inner.height.saturating_sub(details_height),
        );
        diff_scroll = 0;
    } else {
        diff_scroll = diff_scroll.saturating_sub(details_rows);
    }

    let render_area = Rect::new(
        diff_area.x,
        diff_area.y,
        diff_area.width.saturating_sub(1),
        diff_area.height,
    );
    let (divider, content_file_hits) = if render_area.width < 2 || render_area.height == 0 {
        (None, Vec::new())
    } else if let Some(rows) = side_rows.as_deref() {
        let (divider, hits) =
            draw_side_by_side_diff(frame, render_area, app, rows, diff_scroll, theme);
        (Some(divider), hits)
    } else if let Some(rows) = unified_rows.as_deref() {
        let hits = draw_unified_diff(frame, render_area, app, rows, diff_scroll, theme);
        (None, hits)
    } else {
        (None, Vec::new())
    };
    draw_scrollbar(frame, inner, app.content_scroll, visual_length, theme);
    (divider, content_file_hits, Vec::new())
}

fn commit_details_row_count(available_height: u16) -> usize {
    7.min(available_height.saturating_sub(3)) as usize
}

fn pull_request_details_row_count(available_height: u16) -> usize {
    12.min(available_height.saturating_sub(3)) as usize
}

fn draw_commit_details_scrolled(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    scroll: usize,
    total_rows: usize,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) {
    let Some(details) = app.document.commit_details.as_ref() else {
        return;
    };
    let document = &app.document;
    let load_progress = app.local_diff_load_progress();
    let (additions, deletions) = app
        .local_diff_line_counts()
        .unwrap_or_else(|| (document.addition_count(), document.deletion_count()));
    let block = Block::default()
        .title(" Commit details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.panel_alt).fg(theme.text));
    let full_area = Rect::new(0, 0, area.width, cells(total_rows));
    let mut buffer = ratatui::buffer::Buffer::empty(full_area);
    let inner = block.inner(full_area);
    block.render(full_area, &mut buffer);
    let file_count = document.file_count();
    let subject = pull_request_reference(&details.subject)
        .and_then(|(start, end, number)| {
            Some((
                details.subject.get(..start)?,
                details.subject.get(start..end)?,
                details.subject.get(end..)?,
                app.pull_request_open_target(number)?,
            ))
        })
        .map_or_else(
            || {
                Line::from(Span::styled(
                    details.subject.as_str(),
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ))
            },
            |(prefix, reference, suffix, target)| {
                Line::from(vec![
                    Span::styled(
                        prefix,
                        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                    ),
                    interactive_span(
                        reference.to_owned(),
                        Some(target),
                        scrolled_detail_link_area(
                            area,
                            scroll,
                            inner.y,
                            inner.x.saturating_add(cells(prefix.width())),
                            reference.width(),
                        ),
                        theme,
                        link_hits,
                    ),
                    Span::styled(
                        suffix,
                        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                    ),
                ])
            },
        );
    let lines = vec![
        subject,
        detail_line(
            "Author",
            format!(
                "{} <{}>  ·  {}",
                details.author,
                details.author_email,
                format_local_timestamp(&details.authored_at)
            ),
            theme,
        ),
        detail_line(
            "Committer",
            format!(
                "{} <{}>  ·  {}",
                details.committer,
                details.committer_email,
                format_local_timestamp(&details.committed_at)
            ),
            theme,
        ),
        Line::from(vec![
            Span::styled(
                format!("{:<DETAIL_LABEL_WIDTH$}", "Commit"),
                Style::default().fg(theme.muted),
            ),
            interactive_span(
                details.id.clone(),
                app.commit_open_target(&details.id),
                scrolled_detail_link_area(
                    area,
                    scroll,
                    inner.y.saturating_add(3),
                    inner.x.saturating_add(cells(DETAIL_LABEL_WIDTH)),
                    details.id.width(),
                ),
                theme,
                link_hits,
            ),
        ]),
        Line::from(vec![
            Span::styled("Changes    ", Style::default().fg(theme.muted)),
            Span::styled(
                format!(
                    "{} file{} changed{}  ",
                    file_count,
                    if file_count == 1 { "" } else { "s" },
                    load_progress.map_or_else(String::new, |(loaded, total)| {
                        format!("  ·  {loaded}/{total} diffs loaded")
                    }),
                ),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("+{additions}"),
                Style::default()
                    .fg(theme.added)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("-{deletions}"),
                Style::default()
                    .fg(theme.removed)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];
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
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
fn draw_pull_request_details_scrolled(
    frame: &mut Frame<'_>,
    area: Rect,
    details: &PullRequestDetails,
    scroll: usize,
    total_rows: usize,
    theme: &Theme,
) {
    let block = Block::default()
        .title(format!(" Pull request #{} · details ", details.number))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.panel_alt).fg(theme.text));
    let full_area = Rect::new(0, 0, area.width, cells(total_rows));
    let mut buffer = ratatui::buffer::Buffer::empty(full_area);
    let inner = block.inner(full_area);
    block.render(full_area, &mut buffer);
    let state = if details.is_draft {
        "DRAFT"
    } else {
        details.state.as_str()
    };
    let head_repository = details.head_repository.as_deref().unwrap_or("deleted fork");
    let mut lines = vec![
        Line::from(Span::styled(
            details.title.as_str(),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )),
        detail_line(
            "Status",
            format!(
                "{state}  ·  @{}  ·  updated {}",
                details.author,
                format_local_timestamp(&details.updated_at)
            ),
            theme,
        ),
    ];
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
    lines.extend([
        detail_line(
            "Source",
            format!(
                "{}:{}{}{}",
                head_repository,
                details.head_ref,
                remote_suffix(&details.head_remotes),
                if details.is_cross_repository {
                    "  ·  fork"
                } else {
                    ""
                }
            ),
            theme,
        ),
        detail_line(
            "Destination",
            format!(
                "{}:{}{}",
                details.base_repository,
                details.base_ref,
                remote_suffix(&details.base_remotes)
            ),
            theme,
        ),
        detail_line("URL", details.url.clone(), theme),
        Line::from(vec![
            Span::styled(
                format!("{:<DETAIL_LABEL_WIDTH$}", "Selected"),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                details
                    .selected_file
                    .as_deref()
                    .unwrap_or("Preparing files"),
                Style::default().fg(theme.text),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("+{}", details.selected_file_additions),
                Style::default()
                    .fg(theme.added)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("-{}", details.selected_file_deletions),
                Style::default()
                    .fg(theme.removed)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
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

fn description_preview_lines(value: &str, width: usize, maximum_lines: usize) -> Vec<String> {
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

fn markdown_preview_text(value: &str) -> String {
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

fn strip_inline_markdown(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '*' | '`'))
        .collect()
}

fn text_preview_lines(value: &str, width: usize, maximum_lines: usize) -> Vec<String> {
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

fn remote_suffix(remotes: &[String]) -> String {
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

fn detail_line<'a>(label: &'a str, value: String, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{label:<DETAIL_LABEL_WIDTH$}"),
            Style::default().fg(theme.muted),
        ),
        Span::styled(value, Style::default().fg(theme.text)),
    ])
}

fn interactive_span(
    text: String,
    target: Option<OpenTarget>,
    area: Rect,
    theme: &Theme,
    hits: &mut Vec<LinkHit>,
) -> Span<'static> {
    let style = if let Some(target) = target {
        if area.width > 0 && area.height > 0 {
            hits.push(LinkHit { area, target });
        }
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    };
    Span::styled(text, style)
}

fn clipped_link_area(x: u16, y: u16, width: usize, container: Rect) -> Rect {
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

fn scrolled_detail_link_area(
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

fn pull_request_reference(text: &str) -> Option<(usize, usize, u64)> {
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

fn unified_row_indices(document: &DiffDocument, app: &App) -> Vec<usize> {
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

fn draw_unified_diff(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    rows: &[usize],
    diff_scroll: usize,
    theme: &Theme,
) -> Vec<ContentFileHit> {
    let first_index = rows.get(diff_scroll).copied().unwrap_or_default();
    let mut in_file = inside_file_before(&app.document, first_index);
    let emphasis = intraline_emphasis(&app.document.lines);
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
                emphasis.get(line_index).and_then(Option::as_ref),
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
fn draw_unified_line(
    frame: &mut Frame<'_>,
    area: Rect,
    line: &DiffLine,
    boxed: bool,
    horizontal_scroll: usize,
    emphasis: Option<&Range<usize>>,
    theme: &Theme,
) {
    let content_area = if boxed {
        draw_file_edges(frame, area, line.kind, theme);
        Rect::new(area.x + 1, area.y, area.width.saturating_sub(2), 1)
    } else {
        area
    };
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
        Paragraph::new(Line::from(spans)).style(line_background(line.kind, theme)),
        content_area,
    );
}

fn inside_file_before(document: &DiffDocument, offset: usize) -> bool {
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

fn sticky_file_header(document: &DiffDocument, line_index: usize) -> Option<&DiffLine> {
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

fn file_header_path(line: &DiffLine) -> Option<&str> {
    line.spans
        .first()
        .map(|span| span.text.split("  · ").next().unwrap_or(span.text.as_str()))
}

fn draw_file_header(frame: &mut Frame<'_>, area: Rect, line: &DiffLine, app: &App, theme: &Theme) {
    if area.width == 0 {
        return;
    }
    let label = line
        .spans
        .first()
        .map(|span| span.text.as_str())
        .unwrap_or_default();
    let disclosure = if !app.preview_files_collapsible() {
        " "
    } else if file_header_path(line).is_some_and(|path| app.preview_file_collapsed(path)) {
        "›"
    } else {
        "⌄"
    };
    let additions = line.spans.get(1).map_or("+0", |span| span.text.as_str());
    let deletions = line.spans.get(2).map_or("-0", |span| span.text.as_str());
    let reserved = 9_usize + additions.width() + deletions.width();
    let label = truncate_middle(label, (area.width as usize).saturating_sub(reserved));
    let fill = (area.width as usize)
        .saturating_sub(reserved)
        .saturating_sub(label.width());
    let selected = file_header_path(line).is_some_and(|path| app.preview_file_selected(path));
    let border = if selected {
        theme.border_focus
    } else {
        theme.border
    };
    let background = if selected {
        theme.selected
    } else {
        theme.panel_alt
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("┌─", Style::default().fg(border)),
            Span::styled(format!(" {disclosure} "), Style::default().fg(theme.muted)),
            Span::styled(
                label,
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled("─".repeat(fill), Style::default().fg(border)),
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
            Span::styled(" ┐", Style::default().fg(border)),
        ]))
        .style(Style::default().bg(background)),
        area,
    );
}

fn draw_file_footer(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    if area.width == 0 {
        return;
    }
    let text = if area.width == 1 {
        "└".to_owned()
    } else {
        format!("└{}┘", "─".repeat(area.width.saturating_sub(2) as usize))
    };
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(theme.border).bg(theme.panel)),
        area,
    );
}

fn draw_file_edges(frame: &mut Frame<'_>, area: Rect, kind: DiffLineKind, theme: &Theme) {
    if area.width < 2 {
        return;
    }
    let background = match kind {
        DiffLineKind::Added => theme.added_background,
        DiffLineKind::Removed => theme.removed_background,
        DiffLineKind::HunkHeader => theme.panel_alt,
        _ => theme.panel,
    };
    let style = Style::default().fg(theme.border).bg(background);
    frame.render_widget(
        Paragraph::new("│").style(style),
        Rect::new(area.x, area.y, 1, 1),
    );
    frame.render_widget(
        Paragraph::new("│").style(style),
        Rect::new(area.right().saturating_sub(1), area.y, 1, 1),
    );
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
fn draw_side_by_side_diff(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    rows: &[SideBySideRow<'_>],
    diff_scroll: usize,
    theme: &Theme,
) -> (Rect, Vec<ContentFileHit>) {
    let content = Rect::new(
        area.x + 1,
        area.y,
        area.width.saturating_sub(2),
        area.height,
    );
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

    let sticky = rows.get(diff_scroll).and_then(|first| match first {
        SideBySideRow::FileHeader(_) | SideBySideRow::FileFooter => None,
        _ => rows
            .get(..diff_scroll)
            .unwrap_or_default()
            .iter()
            .rev()
            .find_map(|row| match row {
                SideBySideRow::FileHeader(header) => Some(*header),
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
            SideBySideRow::FileHeader(line) => {
                draw_file_header(frame, row_area, line, app, theme);
                if let Some(path) = file_header_path(line) {
                    hits.push(ContentFileHit {
                        area: row_area,
                        path: path.into(),
                    });
                }
            }
            SideBySideRow::FileFooter => draw_file_footer(frame, row_area, theme),
            SideBySideRow::Full { line, boxed } => draw_full_width_diff_line(
                frame,
                row_area,
                line,
                *boxed,
                app.horizontal_scroll,
                theme,
            ),
            SideBySideRow::Split(old_line, new_line) => {
                let (old_emphasis, new_emphasis) = paired_intraline_emphasis(*old_line, *new_line);
                draw_diff_side(
                    frame,
                    Rect::new(left.x, y, left.width, 1),
                    *old_line,
                    true,
                    app.horizontal_scroll,
                    old_emphasis.as_ref(),
                    theme,
                );
                frame.render_widget(
                    Paragraph::new("│").style(Style::default().fg(divider_color).bg(theme.panel)),
                    Rect::new(divider.x, y, 1, 1),
                );
                draw_diff_side(
                    frame,
                    Rect::new(right.x, y, right.width, 1),
                    *new_line,
                    false,
                    app.horizontal_scroll,
                    new_emphasis.as_ref(),
                    theme,
                );
                let edge_kind = old_line
                    .or(*new_line)
                    .map_or(DiffLineKind::Context, |line| line.kind);
                draw_file_edges(frame, row_area, edge_kind, theme);
            }
        }
    }
    (divider, hits)
}

enum SideBySideRow<'a> {
    FileHeader(&'a DiffLine),
    FileFooter,
    Full { line: &'a DiffLine, boxed: bool },
    Split(Option<&'a DiffLine>, Option<&'a DiffLine>),
}

fn side_by_side_rows<'a>(document: &'a DiffDocument, app: &App) -> Vec<SideBySideRow<'a>> {
    let mut rows = Vec::new();
    let mut index = 0;
    let mut in_file = false;
    while let Some(line) = document.lines.get(index) {
        match line.kind {
            DiffLineKind::FileHeader => {
                rows.push(SideBySideRow::FileHeader(line));
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
            DiffLineKind::Meta => {
                rows.push(SideBySideRow::Full {
                    line,
                    boxed: in_file,
                });
                index += 1;
            }
            DiffLineKind::Added => {
                rows.push(SideBySideRow::Split(None, Some(line)));
                index += 1;
            }
            DiffLineKind::Context => {
                rows.push(SideBySideRow::Split(Some(line), Some(line)));
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
                let removed = document
                    .lines
                    .get(removed_start..added_start)
                    .unwrap_or_default();
                let added = document.lines.get(added_start..index).unwrap_or_default();
                for pair_index in 0..removed.len().max(added.len()) {
                    rows.push(SideBySideRow::Split(
                        removed.get(pair_index),
                        added.get(pair_index),
                    ));
                }
            }
        }
    }
    rows
}

fn optional_side_by_side_rows<'a>(
    enabled: bool,
    document: &'a DiffDocument,
    app: &App,
) -> Option<Vec<SideBySideRow<'a>>> {
    enabled.then(|| side_by_side_rows(document, app))
}

fn intraline_emphasis(lines: &[DiffLine]) -> Vec<Option<Range<usize>>> {
    let mut emphasis = vec![None; lines.len()];
    let mut index = 0;
    while let Some(line) = lines.get(index) {
        if line.kind != DiffLineKind::Removed {
            index += 1;
            continue;
        }
        let removed_start = index;
        while lines
            .get(index)
            .is_some_and(|line| line.kind == DiffLineKind::Removed)
        {
            index += 1;
        }
        let added_start = index;
        while lines
            .get(index)
            .is_some_and(|line| line.kind == DiffLineKind::Added)
        {
            index += 1;
        }
        let pair_count = (added_start - removed_start).min(index - added_start);
        for pair_index in 0..pair_count {
            let old_index = removed_start + pair_index;
            let new_index = added_start + pair_index;
            let (old_range, new_range) =
                paired_intraline_emphasis(lines.get(old_index), lines.get(new_index));
            if let Some(slot) = emphasis.get_mut(old_index) {
                *slot = old_range;
            }
            if let Some(slot) = emphasis.get_mut(new_index) {
                *slot = new_range;
            }
        }
    }
    emphasis
}

fn paired_intraline_emphasis(
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

fn changed_ranges(old: &str, new: &str) -> (Option<Range<usize>>, Option<Range<usize>>) {
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

fn draw_full_width_diff_line(
    frame: &mut Frame<'_>,
    area: Rect,
    line: &DiffLine,
    boxed: bool,
    horizontal_scroll: usize,
    theme: &Theme,
) {
    let content_area = if boxed {
        draw_file_edges(frame, area, line.kind, theme);
        Rect::new(area.x + 1, area.y, area.width.saturating_sub(2), 1)
    } else {
        area
    };
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
fn draw_diff_side(
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

fn highlight_spans<'a>(
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
            |(r, g, b)| Color::Rgb(r, g, b),
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
fn push_highlight_piece(
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

fn marker_for(kind: DiffLineKind, theme: &Theme) -> (&'static str, Style) {
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

fn line_background(kind: DiffLineKind, theme: &Theme) -> Style {
    match kind {
        DiffLineKind::Added => Style::default().bg(theme.added_background),
        DiffLineKind::Removed => Style::default().bg(theme.removed_background),
        DiffLineKind::HunkHeader => Style::default().bg(theme.panel_alt).fg(theme.accent),
        _ => Style::default().bg(theme.panel),
    }
}

const fn line_foreground(kind: DiffLineKind, theme: &Theme) -> Color {
    match kind {
        DiffLineKind::Added => theme.added,
        DiffLineKind::Removed => theme.removed,
        DiffLineKind::HunkHeader => theme.accent,
        _ => theme.text,
    }
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
fn draw_scrollbar(frame: &mut Frame<'_>, area: Rect, offset: usize, length: usize, theme: &Theme) {
    if length <= area.height as usize || area.width == 0 {
        return;
    }
    let height = area.height as usize;
    let thumb_height = (height * height / length).max(1).min(height);
    let max_offset = length.saturating_sub(height).max(1);
    let thumb_start = offset.min(max_offset) * (height - thumb_height) / max_offset;
    for row in 0..height {
        let color = if (thumb_start..thumb_start + thumb_height).contains(&row) {
            theme.accent_soft
        } else {
            theme.border
        };
        frame.render_widget(
            Paragraph::new("▐").style(Style::default().fg(color)),
            Rect::new(area.right().saturating_sub(1), area.y + cells(row), 1, 1),
        );
    }
}

#[expect(
    clippy::option_if_let_else,
    reason = "the branch is one arm of a longer chain that map_or_else cannot express"
)]
fn draw_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
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
                " ⟳ ",
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
        Line::from(vec![
            Span::styled(" ⟳ ", Style::default().fg(theme.accent)),
            Span::styled("Refreshing repository…", Style::default().fg(theme.muted)),
        ])
    } else {
        let branch = if app.status.branch.head.is_empty() {
            "—".to_owned()
        } else {
            app.status.branch.head.clone()
        };
        Line::from(vec![
            Span::styled("  ", Style::default().fg(theme.accent)),
            interactive_span(
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
            Span::styled(
                format!(
                    "   {} changes   {} staged",
                    app.status.changes.len(),
                    app.status.staged_count()
                ),
                Style::default().fg(theme.muted),
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

fn draw_modal(frame: &mut Frame<'_>, modal: &Modal, app: &App, theme: &Theme) {
    match modal {
        Modal::Help { scroll } => draw_help(frame, *scroll, theme),
        Modal::Commit { input, amend } => draw_commit(frame, input, *amend, theme),
        Modal::Prompt { title, input, .. } => draw_prompt(frame, title, input, theme),
        Modal::Confirm { title, message, .. } => draw_confirm(frame, title, message, theme),
        Modal::Branches {
            items,
            selected,
            query,
            loading,
            ..
        } => draw_branches(frame, items, *selected, query, *loading, theme),
        Modal::HistoryBranches {
            items,
            selected,
            query,
            loading,
        } => draw_history_branches(frame, items, *selected, query, *loading, theme),
        Modal::CompareBranches {
            items,
            selected,
            query,
            loading,
        } => draw_compare_branches(frame, items, *selected, query, *loading, theme),
        Modal::Stashes {
            items,
            selected,
            query,
            loading,
        } => draw_stashes(frame, items, *selected, query, *loading, theme),
        Modal::PullRequestRepositories {
            items,
            selected,
            query,
            loading,
        } => draw_pull_request_repositories(frame, items, *selected, query, *loading, theme),
        Modal::CommandPalette { query, selected } => {
            draw_palette(frame, app, query, *selected, theme);
        }
        Modal::Conflict { change } => draw_conflict(frame, change, theme),
    }
}

fn draw_help(frame: &mut Frame<'_>, scroll: usize, theme: &Theme) {
    let area = centered_rect(
        68,
        31.min(frame.area().height.saturating_sub(4)),
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Keyboard Shortcuts ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = HELP_LINES
        .iter()
        .skip(scroll)
        .take(inner.height.saturating_sub(1) as usize)
        .map(|(key, description)| {
            if description.is_empty() && !key.is_empty() {
                Line::from(Span::styled(
                    *key,
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(vec![
                    Span::styled(format!("{key:<20}"), Style::default().fg(theme.modified)),
                    Span::styled(*description, Style::default().fg(theme.text)),
                ])
            }
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
    draw_modal_hint(frame, area, "Esc close", theme);
}

fn draw_commit(frame: &mut Frame<'_>, input: &crate::app::TextBuffer, amend: bool, theme: &Theme) {
    let width = frame.area().width.saturating_sub(12).min(76);
    let area = centered_rect(width, 12, frame.area());
    frame.render_widget(Clear, area);
    let block = modal_block(
        if amend {
            " Amend Commit "
        } else {
            " Commit Staged Changes "
        },
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let input_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(3),
    );
    frame.render_widget(
        Paragraph::new(input.value.as_str())
            .style(Style::default().fg(theme.text).bg(theme.panel_alt))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_focus)),
            ),
        input_area,
    );
    set_text_cursor(frame, input_area.inner(Margin::new(1, 1)), input, true);
    frame.render_widget(
        Paragraph::new("Ctrl+Enter commit   Enter newline   Esc cancel")
            .style(Style::default().fg(theme.muted)),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn draw_prompt(frame: &mut Frame<'_>, title: &str, input: &crate::app::TextBuffer, theme: &Theme) {
    let area = centered_rect(
        frame.area().width.saturating_sub(14).min(68),
        7,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(&format!(" {title} "), theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let input_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    frame.render_widget(
        Paragraph::new(input.value.as_str())
            .style(Style::default().fg(theme.text).bg(theme.panel_alt)),
        input_area,
    );
    set_text_cursor(frame, input_area, input, false);
    draw_modal_hint(frame, area, "Enter accept   Esc cancel", theme);
}

fn draw_confirm(frame: &mut Frame<'_>, title: &str, message: &str, theme: &Theme) {
    let area = centered_rect(
        frame.area().width.saturating_sub(14).min(72),
        9,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(&format!(" {title} "), theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(theme.text))
            .wrap(Wrap { trim: true }),
        Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(3),
        ),
    );
    draw_modal_hint(frame, area, "y / Enter confirm   n / Esc cancel", theme);
}

fn draw_conflict(frame: &mut Frame<'_>, change: &Change, theme: &Theme) {
    let area = centered_rect(
        frame.area().width.saturating_sub(14).min(72),
        10,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Resolve Merge Conflict ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                change.display_path(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "o",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" accept ours     "),
                Span::styled(
                    "t",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" accept theirs     "),
                Span::styled(
                    "s",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" mark resolved"),
            ]),
        ]),
        Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(2),
        ),
    );
    draw_modal_hint(frame, area, "Esc cancel", theme);
}

fn draw_branches(
    frame: &mut Frame<'_>,
    items: &[Branch],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    theme: &Theme,
) {
    let height = frame.area().height.saturating_sub(8).min(25);
    let area = centered_rect(
        frame.area().width.saturating_sub(12).min(76),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Branches ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );

    let visible = App::filtered_branches(items, &query.value);
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    if loading {
        frame.render_widget(
            Paragraph::new("Loading branches…").style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
        let lines = visible
            .iter()
            .skip(offset)
            .take(list_area.height as usize)
            .enumerate()
            .filter_map(|(visible_offset, index)| {
                let branch = items.get(*index)?;
                let cursor = offset + visible_offset;
                let style = if cursor == selected {
                    Style::default()
                        .bg(theme.selected)
                        .fg(theme.text)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text)
                };
                Some(Line::from(vec![
                    Span::styled(
                        if branch.current { " ● " } else { "   " },
                        Style::default()
                            .fg(if branch.current {
                                theme.success
                            } else {
                                theme.muted
                            })
                            .bg(style.bg.unwrap_or(theme.panel)),
                    ),
                    Span::styled(
                        truncate_middle(&branch.name, list_area.width.saturating_sub(30) as usize),
                        style,
                    ),
                    Span::styled(
                        format!(
                            "  {}  {}",
                            branch.short_id,
                            format_local_timestamp(&branch.relative_date)
                        ),
                        Style::default()
                            .fg(theme.muted)
                            .bg(style.bg.unwrap_or(theme.panel)),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(lines), list_area);
    }
    draw_modal_hint(
        frame,
        area,
        "Enter switch   Ctrl+n new   F2/Ctrl+r rename   Delete delete   Esc close",
        theme,
    );
}

fn draw_history_branches(
    frame: &mut Frame<'_>,
    items: &[HistoryBranch],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    theme: &Theme,
) {
    let height = frame.area().height.saturating_sub(8).min(25);
    let area = centered_rect(
        frame.area().width.saturating_sub(12).min(82),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" View Branch History — no checkout ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    if loading {
        frame.render_widget(
            Paragraph::new("Loading local and remote-tracking branches…")
                .style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let visible = App::filtered_history_branches(items, &query.value);
        let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
        let lines = visible
            .iter()
            .skip(offset)
            .take(list_area.height as usize)
            .enumerate()
            .filter_map(|(visible_offset, index)| {
                let branch = items.get(*index)?;
                let active = offset + visible_offset == selected;
                let background = if active { theme.selected } else { theme.panel };
                Some(Line::from(vec![
                    Span::styled(
                        if branch.current { " ● " } else { "   " },
                        Style::default()
                            .fg(if branch.current {
                                theme.success
                            } else {
                                theme.muted
                            })
                            .bg(background),
                    ),
                    Span::styled(
                        truncate_middle(&branch.name, list_area.width.saturating_sub(34) as usize),
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
                        format!(
                            "  {}  {}  {}",
                            if branch.remote { "remote" } else { "local" },
                            branch.short_id,
                            format_local_timestamp(&branch.relative_date)
                        ),
                        Style::default().fg(theme.muted).bg(background),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(lines), list_area);
    }
    draw_modal_hint(
        frame,
        area,
        "Enter view history (HEAD and worktree stay unchanged)   Esc close",
        theme,
    );
}

fn draw_compare_branches(
    frame: &mut Frame<'_>,
    items: &[HistoryBranch],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    theme: &Theme,
) {
    let height = frame.area().height.saturating_sub(8).min(25);
    let area = centered_rect(
        frame.area().width.saturating_sub(12).min(82),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Compare Current Branch With… ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    if loading {
        frame.render_widget(
            Paragraph::new("Loading local and remote-tracking branches…")
                .style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let visible = App::filtered_history_branches(items, &query.value)
            .into_iter()
            .filter(|index| items.get(*index).is_some_and(|item| !item.current))
            .collect::<Vec<_>>();
        let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
        let lines = visible
            .iter()
            .skip(offset)
            .take(list_area.height as usize)
            .enumerate()
            .filter_map(|(visible_offset, index)| {
                let branch = items.get(*index)?;
                let active = offset + visible_offset == selected;
                let background = if active { theme.selected } else { theme.panel };
                Some(Line::from(vec![
                    Span::styled(
                        if active { " › " } else { "   " },
                        Style::default().fg(theme.accent).bg(background),
                    ),
                    Span::styled(
                        truncate_middle(&branch.name, list_area.width.saturating_sub(34) as usize),
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
                        format!(
                            "  {}  {}  {}",
                            if branch.remote { "remote" } else { "local" },
                            branch.short_id,
                            format_local_timestamp(&branch.relative_date)
                        ),
                        Style::default().fg(theme.muted).bg(background),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(lines), list_area);
        if visible.is_empty() {
            frame.render_widget(
                Paragraph::new("No other branches match this filter")
                    .style(Style::default().fg(theme.muted)),
                list_area,
            );
        }
    }
    draw_modal_hint(
        frame,
        area,
        "Enter calculate diff against HEAD   Esc close",
        theme,
    );
}

fn draw_stashes(
    frame: &mut Frame<'_>,
    items: &[Stash],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    theme: &Theme,
) {
    let height = frame.area().height.saturating_sub(6).min(27);
    let area = centered_rect(
        frame.area().width.saturating_sub(10).min(88),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Stashes ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(5),
    );
    if loading {
        frame.render_widget(
            Paragraph::new("Loading stashes…").style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let visible = App::filtered_stashes(items, &query.value);
        let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
        let lines = visible
            .iter()
            .skip(offset)
            .take(list_area.height as usize)
            .enumerate()
            .filter_map(|(visible_offset, index)| {
                let stash = items.get(*index)?;
                let active = offset + visible_offset == selected;
                let background = if active { theme.selected } else { theme.panel };
                let branch = if stash.branch.is_empty() {
                    String::new()
                } else {
                    format!(" on {}", stash.branch)
                };
                Some(Line::from(vec![
                    Span::styled(
                        if active { " › " } else { "   " },
                        Style::default().fg(theme.accent).bg(background),
                    ),
                    Span::styled(
                        format!("{}  ", stash.reference),
                        Style::default()
                            .fg(theme.modified)
                            .bg(background)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        truncate_middle(
                            &stash.message,
                            list_area.width.saturating_sub(40) as usize,
                        ),
                        Style::default().fg(theme.text).bg(background),
                    ),
                    Span::styled(
                        format!(
                            "{branch}  {}  {}",
                            stash.short_id,
                            format_local_timestamp(&stash.relative_date)
                        ),
                        Style::default().fg(theme.muted).bg(background),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(lines), list_area);
        if visible.is_empty() {
            frame.render_widget(
                Paragraph::new(if items.is_empty() {
                    "No stashes — Ctrl+n creates one"
                } else {
                    "No stashes match this filter"
                })
                .style(Style::default().fg(theme.muted)),
                list_area,
            );
        }
    }
    draw_modal_hint(
        frame,
        area,
        "Enter preview  Ctrl+n new  Ctrl+u +untracked  Ctrl+s staged  Alt+a apply  Alt+p pop  Del drop",
        theme,
    );
}

fn draw_pull_request_repositories(
    frame: &mut Frame<'_>,
    items: &[GitHubRepository],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    theme: &Theme,
) {
    let height = (cells(items.len()) + 7)
        .min(frame.area().height.saturating_sub(8))
        .max(10);
    let area = centered_rect(
        frame.area().width.saturating_sub(12).min(82),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Pull Request Repository ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    let visible = App::filtered_github_repositories(items, &query.value);
    let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
    let lines = visible
        .iter()
        .skip(offset)
        .take(list_area.height as usize)
        .enumerate()
        .filter_map(|(visible_offset, index)| {
            let repository = items.get(*index)?;
            let active = offset + visible_offset == selected;
            let background = if active { theme.selected } else { theme.panel };
            let remotes = if repository.remotes.is_empty() {
                "inferred".to_owned()
            } else {
                repository.remotes.join(", ")
            };
            Some(Line::from(vec![
                Span::styled(
                    if active { " › " } else { "   " },
                    Style::default().fg(theme.accent).bg(background),
                ),
                Span::styled(
                    truncate_middle(
                        &repository.display_name(),
                        list_area.width.saturating_sub(24) as usize,
                    ),
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
                    format!("  remote {remotes}"),
                    Style::default().fg(theme.muted).bg(background),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    if loading {
        frame.render_widget(
            Paragraph::new("Discovering GitHub repositories from configured remotes…")
                .style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        frame.render_widget(Paragraph::new(lines), list_area);
    }
    draw_modal_hint(
        frame,
        area,
        if loading {
            "Discovering repositories…   Esc close"
        } else {
            "Enter select repository and reopen only the entered PR   Esc close"
        },
        theme,
    );
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
fn draw_palette(
    frame: &mut Frame<'_>,
    app: &App,
    query: &crate::app::TextBuffer,
    selected: usize,
    theme: &Theme,
) {
    let commands = app.palette_commands(&query.value);
    let height = (cells(commands.len()) + 6)
        .min(frame.area().height.saturating_sub(6))
        .max(8);
    let area = Rect::new(
        frame.area().x
            + (frame
                .area()
                .width
                .saturating_sub(76.min(frame.area().width.saturating_sub(8))))
                / 2,
        frame.area().y + 3,
        76.min(frame.area().width.saturating_sub(8)),
        height,
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Command Palette ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" > ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(2),
    );
    let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
    let lines = commands
        .iter()
        .skip(offset)
        .take(list_area.height as usize)
        .enumerate()
        .map(|(index, command)| palette_line(*command, offset + index == selected, theme))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), list_area);
}

fn palette_line(command: PaletteCommand, selected: bool, theme: &Theme) -> Line<'static> {
    let background = if selected {
        theme.selected
    } else {
        theme.panel
    };
    Line::from(vec![
        Span::styled(
            if selected { " › " } else { "   " },
            Style::default().fg(theme.accent).bg(background),
        ),
        Span::styled(
            command.label(),
            Style::default()
                .fg(theme.text)
                .bg(background)
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
    ])
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
fn progress_bar(percent: u16, width: usize) -> String {
    let filled = usize::from(percent.min(100)).saturating_mul(width) / 100;
    format!(
        "{}{}",
        "█".repeat(filled),
        "░".repeat(width.saturating_sub(filled))
    )
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
fn draw_toast(frame: &mut Frame<'_>, message: &str, level: ToastLevel, theme: &Theme) {
    let width = (cells(message.width()) + 6)
        .min(frame.area().width.saturating_sub(4))
        .max(24);
    let height = ((cells(message.width()) / width.max(1)) + 3).min(7);
    let area = Rect::new(
        frame.area().right().saturating_sub(width + 2),
        frame.area().bottom().saturating_sub(height + 3),
        width,
        height,
    );
    let color = match level {
        ToastLevel::Info => theme.accent,
        ToastLevel::Success => theme.success,
        ToastLevel::Error => theme.error,
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(theme.text).bg(theme.panel_alt))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(color)),
            ),
        area,
    );
}

fn draw_modal_hint(frame: &mut Frame<'_>, area: Rect, hint: &str, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(hint)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.muted).bg(theme.panel)),
        Rect::new(
            area.x + 2,
            area.bottom().saturating_sub(2),
            area.width.saturating_sub(4),
            1,
        ),
    );
}

fn set_text_cursor(
    frame: &mut Frame<'_>,
    area: Rect,
    input: &crate::app::TextBuffer,
    multiline: bool,
) {
    let before = input.before_cursor();
    let (row, column) = if multiline {
        let row = before
            .chars()
            .filter(|character| *character == '\n')
            .count();
        let column = before.rsplit('\n').next().unwrap_or_default().width();
        (row, column)
    } else {
        (0, before.replace('\n', " ").width())
    };
    let x = area
        .x
        .saturating_add(cells(column.min(area.width.saturating_sub(1) as usize)));
    let y = area
        .y
        .saturating_add(cells(row.min(area.height.saturating_sub(1) as usize)));
    frame.set_cursor_position((x, y));
}

fn panel_block(title: String, focused: bool, theme: &Theme) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(if focused {
            theme.border_focus
        } else {
            theme.border
        }))
        .style(Style::default().bg(theme.panel).fg(theme.text))
}

fn modal_block(title: &str, theme: &Theme) -> Block<'static> {
    Block::default()
        .title(title.to_owned())
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_focus))
        .style(Style::default().bg(theme.panel).fg(theme.text))
}

const fn status_color(status: ChangeStatus, theme: &Theme) -> Color {
    match status {
        ChangeStatus::Added | ChangeStatus::Untracked => theme.added,
        ChangeStatus::Deleted => theme.removed,
        ChangeStatus::Conflicted => theme.conflict,
        ChangeStatus::Modified
        | ChangeStatus::Renamed
        | ChangeStatus::Copied
        | ChangeStatus::TypeChanged => theme.modified,
    }
}

const fn graph_color(index: usize, theme: &Theme) -> Color {
    match index % 4 {
        0 => theme.accent,
        1 => theme.modified,
        2 => theme.added,
        _ => theme.conflict,
    }
}

const fn history_glyph(commit: &crate::git::history::Commit, index: usize) -> &'static str {
    if commit.parent_ids.len() > 1 {
        "●╮ "
    } else if index > 0 {
        "●│ "
    } else {
        "●  "
    }
}

fn clean_decoration(decoration: &str) -> &str {
    decoration.strip_prefix("HEAD -> ").unwrap_or(decoration)
}

fn ensure_offset(offset: &mut usize, cursor: usize, height: usize, length: usize) {
    if height == 0 || length == 0 {
        *offset = 0;
        return;
    }
    if cursor < *offset {
        *offset = cursor;
    } else if cursor >= *offset + height {
        *offset = cursor + 1 - height;
    }
    *offset = (*offset).min(length.saturating_sub(height));
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let width = width.min(outer.width);
    let height = height.min(outer.height);
    Rect::new(
        outer.x + (outer.width - width) / 2,
        outer.y + (outer.height - height) / 2,
        width,
        height,
    )
}

fn truncate_end(value: &str, width: usize) -> String {
    if value.width() <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_owned();
    }
    format!("{}…", slice_width(value, 0, width - 1))
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
fn truncate_middle(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if value.width() <= width {
        return value.to_owned();
    }
    if width <= 1 {
        return "…".to_owned();
    }
    let left_width = (width - 1) * 2 / 3;
    let right_width = width - 1 - left_width;
    format!(
        "{}…{}",
        slice_width(value, 0, left_width),
        suffix_width(value, right_width)
    )
}

fn slice_width(value: &str, skip: usize, width: usize) -> String {
    let mut skipped = 0;
    let mut used = 0;
    let mut output = String::new();
    for character in value.chars() {
        let character_width = character.width().unwrap_or_default();
        if skipped + character_width <= skip {
            skipped += character_width;
            continue;
        }
        if used + character_width > width {
            break;
        }
        output.push(character);
        used += character_width;
    }
    output
}

fn suffix_width(value: &str, width: usize) -> String {
    let mut characters = Vec::new();
    let mut used = 0;
    for character in value.chars().rev() {
        let character_width = character.width().unwrap_or_default();
        if used + character_width > width {
            break;
        }
        characters.push(character);
        used += character_width;
    }
    characters.into_iter().rev().collect()
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use super::*;
    use crate::git::diff::CommitDetails;

    #[test]
    fn three_tabs_fit_the_minimum_supported_terminal_width() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new("/tmp/repo", "repo");
        app.status.branch.head = "feature/a-very-long-branch-name".to_owned();
        let backend = TestBackend::new(72, 18);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        assert!(app.geometry.changes_tab.width > 0);
        assert!(app.geometry.history_tab.x > app.geometry.changes_tab.x);
        assert!(app.geometry.pull_requests_tab.x > app.geometry.history_tab.x);
        assert!(app.geometry.pull_requests_tab.right() <= 72);
    }

    #[test]
    fn header_registers_repository_branch_and_workspace_links() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new("/tmp/repo", "repo");
        app.status.branch.head = "feature/link".to_owned();
        app.local_github_repository = Some(GitHubRepository {
            name_with_owner: "acme/repo".to_owned(),
            url: "https://github.com/acme/repo".to_owned(),
            remotes: vec!["origin".to_owned()],
        });
        let backend = TestBackend::new(160, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        assert!(app.geometry.link_hits.iter().any(|hit| matches!(
            &hit.target,
            OpenTarget::Browser(url) if url == "https://github.com/acme/repo"
        )));
        assert!(app.geometry.link_hits.iter().any(|hit| matches!(
            &hit.target,
            OpenTarget::Browser(url)
                if url == "https://github.com/acme/repo/tree/feature/link"
        )));
        assert!(app.geometry.link_hits.iter().any(|hit| matches!(
            &hit.target,
            OpenTarget::Path(path) if path == std::path::Path::new("/tmp/repo")
        )));
    }

    #[test]
    fn changes_view_exposes_vscode_style_file_group_and_toolbar_actions() {
        use std::path::PathBuf;

        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new("/tmp/repo", "repo");
        app.status.changes = vec![
            Change {
                path: PathBuf::from("src/main.rs"),
                original_path: None,
                area: ChangeArea::Unstaged,
                status: ChangeStatus::Modified,
            },
            Change {
                path: PathBuf::from("README.md"),
                original_path: None,
                area: ChangeArea::Staged,
                status: ChangeStatus::Modified,
            },
            Change {
                path: PathBuf::from("notes.txt"),
                original_path: None,
                area: ChangeArea::Unstaged,
                status: ChangeStatus::Untracked,
            },
        ];
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(rendered.contains("[+]"));
        assert!(rendered.contains("[−]"));
        assert!(rendered.contains("[c] Commit"));
        assert!(rendered.contains("[S] Stashes"));
        assert!(rendered.contains("[d] Compare Branch"));
        assert!(rendered.contains("UNTRACKED CHANGES"));
        assert!(!rendered.contains('›'));
        assert!(
            app.geometry
                .scm_action_hits
                .iter()
                .any(|hit| matches!(hit.action, ScmAction::Stage(0)))
        );
        assert!(
            app.geometry
                .scm_action_hits
                .iter()
                .any(|hit| matches!(hit.action, ScmAction::Unstage(1)))
        );
    }

    #[test]
    fn middle_truncation_respects_display_width() {
        let result = truncate_middle("src/a-very-long-file-name.rs", 14);
        assert!(result.width() <= 14);
        assert!(result.contains('…'));
        assert!(result.ends_with("me.rs"));
    }

    #[test]
    fn side_by_side_pairs_replacements() {
        let document = DiffDocument {
            title: String::new(),
            truncated: false,
            commit_details: None,
            pull_request_details: None,
            lines: vec![
                test_line(DiffLineKind::Removed, "old one"),
                test_line(DiffLineKind::Removed, "old two"),
                test_line(DiffLineKind::Added, "new one"),
                test_line(DiffLineKind::Context, "same"),
            ],
        };
        let app = App::new("/tmp/repo", "repo");
        let rows = side_by_side_rows(&document, &app);
        assert_eq!(rows.len(), 3);
        let SideBySideRow::Split(old, new) = &rows[0] else {
            panic!("expected a split diff row");
        };
        assert_eq!(old.unwrap().text(), "old one");
        assert_eq!(new.unwrap().text(), "new one");
        let SideBySideRow::Split(_, new) = &rows[1] else {
            panic!("expected a split diff row");
        };
        assert!(new.is_none());
        let SideBySideRow::Split(old, _) = &rows[2] else {
            panic!("expected a split diff row");
        };
        assert_eq!(old.unwrap().text(), "same");
    }

    #[test]
    fn pull_request_folders_render_as_clickable_collapse_controls() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new("/tmp/repo", "repo");
        app.pull_request_files = ["src/app.rs", "src/git/diff.rs"]
            .into_iter()
            .map(|path| PullRequestFile {
                path: std::path::PathBuf::from(path),
                old_path: None,
                status: PullRequestFileStatus::Modified,
                counts: None,
            })
            .collect();
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut hits = Vec::new();

        terminal
            .draw(|frame| {
                hits =
                    draw_pull_request_file_tree(frame, frame.area(), &mut app, &Theme::default());
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains("⌄ src/"));
        assert!(rendered.contains("app.rs"));
        assert!(hits.iter().any(|hit| {
            matches!(
                &hit.target,
                SidebarHit::PullRequestDirectory(path) if path == std::path::Path::new("src")
            )
        }));

        app.collapsed_pull_request_directories
            .insert(std::path::PathBuf::from("src"));
        app.pull_request_tree.clear();
        terminal.clear().unwrap();
        terminal
            .draw(|frame| {
                draw_pull_request_file_tree(frame, frame.area(), &mut app, &Theme::default());
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains("› src/"));
        assert!(!rendered.contains("app.rs"));
    }

    #[test]
    fn pull_request_file_tree_virtualizes_a_thousand_files() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new("/tmp/repo", "repo");
        app.pull_request_files = (0..1_000)
            .map(|index| PullRequestFile {
                path: std::path::PathBuf::from(format!(
                    "packages/package-{index:04}/src/file-{index:04}.rs"
                )),
                old_path: None,
                status: PullRequestFileStatus::Modified,
                counts: None,
            })
            .collect();
        app.pull_request_total_files = app.pull_request_files.len();
        app.pull_request_file_cursor = 999;
        let rows = app.pull_request_tree_entries();
        assert_eq!(
            rows.iter()
                .filter(|row| matches!(row, PullRequestTreeEntry::File { .. }))
                .count(),
            1_000
        );
        app.pull_request_tree_cursor = rows
            .iter()
            .position(|row| matches!(row, PullRequestTreeEntry::File { index: 999, .. }))
            .unwrap();

        let backend = TestBackend::new(48, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                draw_pull_request_file_tree(frame, frame.area(), &mut app, &Theme::default());
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(app.sidebar_offset > 0);
        assert!(rendered.contains("file-0999.rs"));
    }

    #[test]
    fn hides_raw_hunk_coordinates_in_both_diff_layouts() {
        let document = DiffDocument {
            title: String::new(),
            truncated: false,
            commit_details: None,
            pull_request_details: None,
            lines: vec![
                test_file_header("src/main.rs", 1, 0),
                test_line(DiffLineKind::HunkHeader, "@@ -10,2 +10,3 @@ fn main()"),
                test_line(DiffLineKind::Context, "same"),
                test_line(DiffLineKind::FileFooter, ""),
            ],
        };

        let app = App::new("/tmp/repo", "repo");
        assert_eq!(unified_row_indices(&document, &app), vec![0, 2, 3]);
        assert!(
            side_by_side_rows(&document, &app)
                .iter()
                .all(|row| !matches!(row, SideBySideRow::Full { line, .. } if line.kind == DiffLineKind::HunkHeader))
        );
    }

    #[test]
    fn commit_preview_renders_details_once_and_names_each_file_pane() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::History;
        app.focus = Focus::Content;
        app.status.branch.head = "main".to_owned();
        app.history_branch = Some(HistoryBranch {
            name: "origin/topic".to_owned(),
            reference: "refs/remotes/origin/topic".to_owned(),
            current: false,
            remote: true,
            relative_date: "now".to_owned(),
            short_id: "abc1234".to_owned(),
        });
        app.document = DiffDocument {
            title: "abc1234 — Improve history".to_owned(),
            truncated: false,
            commit_details: Some(CommitDetails {
                id: "abc123456789".to_owned(),
                subject: "Improve history".to_owned(),
                author: "Ada".to_owned(),
                author_email: "ada@example.com".to_owned(),
                authored_at: "2026-01-02T03:04:05Z".to_owned(),
                committer: "Grace".to_owned(),
                committer_email: "grace@example.com".to_owned(),
                committed_at: "2026-01-02T04:05:06Z".to_owned(),
            }),
            pull_request_details: None,
            lines: vec![
                test_file_header("src/main.rs", 1, 0),
                test_line(DiffLineKind::HunkHeader, "@@ -1,0 +1 @@"),
                test_line(DiffLineKind::Added, "fn main() {}"),
                test_line(DiffLineKind::FileFooter, ""),
                test_file_header("README.md", 1, 0),
                test_line(DiffLineKind::HunkHeader, "@@ -1,0 +1 @@"),
                test_line(DiffLineKind::Added, "# Quinjet"),
                test_line(DiffLineKind::FileFooter, ""),
            ],
        };
        let backend = TestBackend::new(140, 32);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert_eq!(rendered.matches("Commit details").count(), 1);
        assert!(rendered.contains("origin/topic"));
        assert!(rendered.contains("[b branch]"));
        assert!(rendered.contains("src/main.rs"));
        assert!(rendered.contains("README.md"));
        assert!(!rendered.contains("@@"));
        assert!(!rendered.contains('◆'));
        assert!(!rendered.contains('░'));
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the draw pass reads better as one top-to-bottom pass"
    )]
    #[test]
    fn pull_request_preview_renders_cross_remote_metadata_and_diff() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::PullRequests;
        app.focus = Focus::Content;
        app.pull_request_section = PullRequestSection::Files;
        app.pull_request_exact_number = Some(42);
        app.pull_request_lookup = crate::app::TextBuffer::new("42");
        app.pull_request = Some(crate::git::github::PullRequest {
            number: 42,
            title: "Ship the rocket".to_owned(),
            description:
                "## Summary\n- Launch **safely** after all checks pass\n- Keep raw `gh` output bounded"
                    .to_owned(),
            author: "octocat".to_owned(),
            state: "OPEN".to_owned(),
            is_draft: false,
            created_at: String::new(),
            updated_at: String::new(),
            url: "https://github.com/acme/widget/pull/42".to_owned(),
            base_ref: "main".to_owned(),
            base_oid: String::new(),
            head_ref: "feature/rocket".to_owned(),
            head_oid: String::new(),
            base_repository: GitHubRepository {
                name_with_owner: "acme/widget".to_owned(),
                url: "https://github.com/acme/widget".to_owned(),
                remotes: vec!["upstream".to_owned()],
            },
            head_repository: Some("octocat/widget".to_owned()),
            head_remotes: vec!["origin".to_owned(), "publish".to_owned()],
            is_cross_repository: true,
            additions: 101,
            deletions: 20,
            changed_files: 1,
        });
        app.pull_request_repository = Some(GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: vec!["upstream".to_owned()],
        });
        app.pull_request_files = vec![PullRequestFile {
            path: std::path::PathBuf::from("src/rocket.rs"),
            old_path: None,
            status: PullRequestFileStatus::Added,
            counts: None,
        }];
        app.pull_request_total_files = 1;
        app.document = DiffDocument {
            title: "PR #42 — Ship the rocket".to_owned(),
            truncated: false,
            commit_details: None,
            pull_request_details: Some(PullRequestDetails {
                number: 42,
                title: "Ship the rocket".to_owned(),
                description: "Launch safely after all checks pass".to_owned(),
                author: "octocat".to_owned(),
                state: "OPEN".to_owned(),
                is_draft: false,
                updated_at: "2026-08-13T12:00:00Z".to_owned(),
                url: "https://github.com/acme/widget/pull/42".to_owned(),
                base_repository: "acme/widget".to_owned(),
                base_ref: "main".to_owned(),
                base_remotes: vec!["upstream".to_owned()],
                head_repository: Some("octocat/widget".to_owned()),
                head_ref: "feature/rocket".to_owned(),
                head_remotes: vec!["origin".to_owned(), "publish".to_owned()],
                is_cross_repository: true,
                changed_files: 1,
                additions: 101,
                deletions: 20,
                selected_file: Some("src/rocket.rs".to_owned()),
                selected_file_additions: 1,
                selected_file_deletions: 0,
            }),
            lines: vec![
                test_file_header("src/rocket.rs", 1, 0),
                test_line(DiffLineKind::HunkHeader, "@@ -0,0 +1 @@"),
                test_line(DiffLineKind::Added, "launch();"),
                test_line(DiffLineKind::FileFooter, ""),
            ],
        };
        let backend = TestBackend::new(160, 34);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(rendered.contains("Pull Requests"));
        assert!(rendered.contains("Files 1"));
        assert!(rendered.contains("PR"));
        assert!(rendered.contains("rocket.rs"));
        assert!(rendered.contains("launch();"));
        assert!(!rendered.contains("Page"));
        assert!(!rendered.contains("files on page"));
        assert!(!rendered.contains("@@"));

        app.pull_request_section = PullRequestSection::Overview;
        app.pull_request_checks = vec![PullRequestCheck {
            name: "CI / ubuntu".to_owned(),
            workflow: "CI".to_owned(),
            state: "SUCCESS".to_owned(),
            status: PullRequestCheckStatus::Passed,
            description: "All jobs passed".to_owned(),
            link: "https://github.com/acme/widget/actions/1".to_owned(),
            started_at: "2026-08-13T12:00:00Z".to_owned(),
            completed_at: "2026-08-13T12:01:00Z".to_owned(),
        }];
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains("Conversation"));
        assert!(rendered.contains("PR ↻"));
        assert!(rendered.contains("Conversation  ↻"));
        assert!(
            app.geometry
                .sidebar_hits
                .iter()
                .any(|hit| matches!(hit.target, SidebarHit::PullRequestRefresh))
        );
        assert!(
            app.geometry
                .sidebar_hits
                .iter()
                .any(|hit| matches!(hit.target, SidebarHit::PullRequestConversationRefresh))
        );
        assert!(rendered.contains("CI / ubuntu"));
        assert!(rendered.contains("Ship the rocket"));
        assert!(rendered.contains("octocat/widget:feature/rocket"));
        assert!(rendered.contains("acme/widget:main"));
        assert!(rendered.contains("+101"));
        assert!(rendered.contains("-20"));
        assert!(
            rendered.contains("Launch"),
            "the pull-request body is part of the default view"
        );

        let refresh_area = app
            .geometry
            .sidebar_hits
            .iter()
            .find(|hit| matches!(hit.target, SidebarHit::PullRequestConversationRefresh))
            .expect("the conversation refresh control has a hit area")
            .area;
        let effects = app.handle_mouse(
            crossterm::event::MouseEvent {
                kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                column: refresh_area.x,
                row: refresh_area.y,
                modifiers: crossterm::event::KeyModifiers::NONE,
            },
            std::time::Instant::now(),
        );
        assert!(effects.iter().any(|effect| matches!(
            effect,
            crate::app::AppEffect::Git(command)
                if matches!(
                    command.as_ref(),
                    crate::git::worker::WorkerCommand::LoadPullRequestConversation { .. }
                )
        )));
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains("PR ⟳"));
        assert!(rendered.contains("Conversation  ⟳"));
    }

    fn overview_app() -> App {
        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::PullRequests;
        app.pull_request_exact_number = Some(42);
        app.pull_request = Some(crate::git::github::PullRequest {
            number: 42,
            title: "Ship the rocket".to_owned(),
            description: "## Summary\n- Launch **safely**\n\n```sh\ncargo test\n```".to_owned(),
            author: "octocat".to_owned(),
            state: "OPEN".to_owned(),
            is_draft: false,
            created_at: "2026-08-01T09:00:00Z".to_owned(),
            updated_at: "2026-08-02T10:30:00Z".to_owned(),
            url: "https://github.com/acme/widget/pull/42".to_owned(),
            base_ref: "main".to_owned(),
            base_oid: String::new(),
            head_ref: "feature/rocket".to_owned(),
            head_oid: String::new(),
            base_repository: GitHubRepository {
                name_with_owner: "acme/widget".to_owned(),
                url: "https://github.com/acme/widget".to_owned(),
                remotes: vec!["origin".to_owned()],
            },
            head_repository: Some("acme/widget".to_owned()),
            head_remotes: vec!["origin".to_owned()],
            is_cross_repository: false,
            additions: 101,
            deletions: 20,
            changed_files: 3,
        });
        app.pull_request_checks = vec![PullRequestCheck {
            name: "Format, lint, and test".to_owned(),
            workflow: "CI".to_owned(),
            state: "FAILURE".to_owned(),
            status: PullRequestCheckStatus::Failed,
            description: String::new(),
            link: "https://github.com/acme/widget/actions/runs/9/job/12".to_owned(),
            started_at: "2026-08-02T10:00:00Z".to_owned(),
            completed_at: "2026-08-02T10:02:30Z".to_owned(),
        }];
        app
    }

    #[test]
    fn a_long_conversation_stays_bounded_to_render() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = overview_app();
        let body = "Some reasonably long review comment body that wraps across several \
terminal rows because that is what real pull-request comments look like in practice."
            .to_owned();
        app.pull_request_conversation = crate::git::github::PullRequestConversation {
            truncated: true,
            from_cache: false,
            entries: (0..500)
                .map(|index| ConversationEntry {
                    kind: if index % 3 == 0 {
                        ConversationKind::Comment
                    } else {
                        ConversationKind::Commit
                    },
                    actor: "octocat".to_owned(),
                    timestamp: "2026-08-02T09:10:00Z".to_owned(),
                    detail: "abc1234".to_owned(),
                    body: body.clone(),
                    url: String::new(),
                    reference: String::new(),
                    context: String::new(),
                })
                .collect(),
        };

        let rows = conversation_rows(&app, 120, &Theme::default());

        assert!(
            rows.len() < 3_000,
            "a thread at the fetch cap still builds a bounded number of rows: {}",
            rows.len()
        );
        assert!(
            rows.iter()
                .any(|row| row.line.to_string().contains("Older activity was omitted")),
            "a truncated thread says so rather than silently dropping history"
        );

        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let cache_key = app.pull_request_content_rows_key;
        let cache_pointer = app.pull_request_content_rows.as_ptr();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        assert_eq!(app.pull_request_content_rows_key, cache_key);
        assert_eq!(app.pull_request_content_rows.as_ptr(), cache_pointer);
    }

    #[test]
    fn a_large_check_log_scrolls_from_a_cached_layout() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = overview_app();
        app.pull_request_check_cursor = Some(0);
        assert!(app.expanded_check_steps.insert(1));
        app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
            steps: vec![CheckStep {
                number: 1,
                name: "Large build".to_owned(),
                status: PullRequestCheckStatus::Passed,
                conclusion: "success".to_owned(),
                started_at: String::new(),
                completed_at: String::new(),
                lines: (0..50_000)
                    .map(|index| CheckLogLine {
                        timestamp: String::new(),
                        text: format!("output line {index}"),
                        severity: CheckLogSeverity::Normal,
                    })
                    .collect(),
            }],
            loose_lines: Vec::new(),
            truncated: false,
            unavailable: None,
            log_pending: false,
        });
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        assert!(app.pull_request_content_rows.len() > 50_000);
        let cache_pointer = app.pull_request_content_rows.as_ptr();
        app.content_scroll = usize::MAX;
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert_eq!(app.pull_request_content_rows.as_ptr(), cache_pointer);
        assert!(rendered.contains("output line 49999"));
    }

    #[test]
    fn an_expanded_step_can_be_scrolled_past_to_reach_the_steps_below_it() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = overview_app();
        app.pull_request_check_cursor = Some(0);
        let line = |text: &str| CheckLogLine {
            timestamp: "2026-08-02T10:00:01Z".to_owned(),
            text: text.to_owned(),
            severity: CheckLogSeverity::Normal,
        };
        let step = |number: usize, lines: usize| CheckStep {
            number,
            name: format!("Step {number}"),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: "2026-08-02T10:00:00Z".to_owned(),
            completed_at: "2026-08-02T10:00:05Z".to_owned(),
            lines: (0..lines)
                .map(|index| line(&format!("output line {index}")))
                .collect(),
        };
        app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
            steps: vec![step(1, 300), step(2, 0), step(3, 0)],
            loose_lines: Vec::new(),
            truncated: false,
            unavailable: None,
            log_pending: false,
        });
        app.expanded_check_steps.insert(1);
        app.pull_request_step_cursor = 1;
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        app.content_scroll = 120;
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        assert_eq!(
            app.content_scroll, 120,
            "a redraw must not drag the view back to the selected step"
        );

        app.content_scroll = usize::MAX;
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            rendered.contains("Step 2") && rendered.contains("Step 3"),
            "the steps below a long expanded step are reachable by scrolling"
        );
        assert!(
            !rendered.contains("output line 0 "),
            "the view really moved past the expanded output"
        );
    }

    #[test]
    fn the_selected_step_is_the_row_that_is_highlighted() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = overview_app();
        let theme = Theme::default();
        app.pull_request_check_cursor = Some(0);
        let step = |number: usize| CheckStep {
            number,
            name: format!("Run step {number}"),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: "2026-08-02T10:00:00Z".to_owned(),
            completed_at: "2026-08-02T10:00:01Z".to_owned(),
            lines: Vec::new(),
        };
        app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
            steps: (1..=4).map(step).collect(),
            loose_lines: Vec::new(),
            truncated: false,
            unavailable: None,
            log_pending: false,
        });
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();

        let highlighted = |app: &mut App, terminal: &mut Terminal<TestBackend>| {
            terminal.draw(|frame| draw(frame, app)).unwrap();
            let buffer = terminal.backend().buffer().clone();
            (0..30)
                .filter(|y| buffer[(60, *y)].style().bg == Some(theme.selected))
                .map(|y| {
                    (44..99)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                        .trim()
                        .to_owned()
                })
                .collect::<Vec<_>>()
        };

        for cursor in [2, 4, 1] {
            app.pull_request_step_cursor = cursor;
            let rows = highlighted(&mut app, &mut terminal);
            assert_eq!(
                rows.len(),
                1,
                "exactly one row is highlighted, not a range: {rows:?}"
            );
            assert!(
                rows[0].contains(&format!("Run step {cursor}")),
                "the highlight marks the step the cursor is on: {rows:?}"
            );
        }
    }

    #[test]
    fn a_running_check_shows_the_step_it_is_on_before_any_log_exists() {
        let mut app = overview_app();
        app.pull_request_check_cursor = Some(0);
        app.expanded_check_steps.insert(2);
        app.pull_request_step_cursor = 2;
        app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
            truncated: false,
            unavailable: None,
            log_pending: true,
            loose_lines: Vec::new(),
            steps: vec![
                CheckStep {
                    number: 1,
                    name: "Set up job".to_owned(),
                    status: PullRequestCheckStatus::Passed,
                    conclusion: "success".to_owned(),
                    started_at: "2026-08-02T10:00:00Z".to_owned(),
                    completed_at: "2026-08-02T10:00:02Z".to_owned(),
                    lines: Vec::new(),
                },
                CheckStep {
                    number: 2,
                    name: "Run cargo test".to_owned(),
                    status: PullRequestCheckStatus::Pending,
                    conclusion: String::new(),
                    started_at: "2026-08-02T10:00:02Z".to_owned(),
                    completed_at: String::new(),
                    lines: Vec::new(),
                },
            ],
        });

        let rendered = check_run_rows(&app, 100, &Theme::default())
            .iter()
            .map(|row| row.line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("2 steps"));
        assert!(
            rendered.contains("Set up job") && rendered.contains("2s"),
            "a finished step still reports how long it took"
        );
        assert!(
            rendered.contains("Run cargo test"),
            "the step in progress is visible rather than hidden behind the missing log"
        );
        assert!(rendered.contains("waiting for output…"));
        assert!(
            rendered.contains("Waiting for the runner"),
            "the view says why there is no output yet instead of looking broken"
        );
    }

    #[test]
    fn a_comment_shows_its_code_intact_and_only_that_code_scrolls() {
        let mut app = overview_app();
        let wide = "│ ✓ Format, lint, and test (ubuntu-latest)   CI   passed in 33s   https://github.com/acme/widget/actions/runs/1/job/2 │";
        app.pull_request_conversation = crate::git::github::PullRequestConversation {
            truncated: false,
            from_cache: false,
            entries: vec![ConversationEntry {
                kind: ConversationKind::Comment,
                actor: "pulkitxm".to_owned(),
                timestamp: "2026-08-14T20:08:00Z".to_owned(),
                detail: String::new(),
                body: format!(
                    "webhook path, driven against this PR with a body long enough to wrap.\n\n```\n{wide}\n  short line\n```"
                ),
                url: String::new(),
                reference: String::new(),
                context: String::new(),
            }],
        };

        let rows = conversation_rows(&app, 80, &Theme::default());
        let text = |row: &ContentRow| row.line.to_string();

        assert!(
            !rows.iter().any(|row| text(row).contains("```")),
            "fence markers are punctuation for a parser, never shown to a reader"
        );
        let scrollable: Vec<String> = rows.iter().filter(|row| row.wide).map(text).collect();
        assert!(scrollable.iter().any(|row| row.contains("cargo test")));
        assert!(scrollable.iter().any(|row| row.contains("  short line")));
        assert!(
            scrollable.iter().any(|row| row.contains("State")
                && row.contains(&format!(
                    "opened {}",
                    format_local_timestamp("2026-08-01T09:00:00Z")
                ))),
            "a single-line value that outgrows the pane scrolls rather than being clipped"
        );
        let long = rows
            .iter()
            .find(|row| row.wide && text(row).contains(wide))
            .expect("code keeps its full width rather than being cut at the pane");
        assert!(
            rows.iter()
                .filter(|row| !row.wide)
                .all(|row| row.line.width() <= 80),
            "everything that is not code is wrapped to fit"
        );

        let prose = rows
            .iter()
            .find(|row| !row.wide && text(row).contains("webhook path"))
            .expect("the comment body is rendered");
        assert_eq!(
            shift_line(&prose.line, 0, 80).to_string(),
            prose.line.to_string()
        );
        let shifted = shift_line(&long.line, 60, 80).to_string();
        assert!(
            shifted.contains("33s") && shifted.contains("job/2"),
            "scrolling reaches the tail of a line the pane cannot hold: {shifted:?}"
        );
        assert!(!shifted.contains("Format, lint"));
        assert!(
            shift_line(&long.line, 0, 80).to_string().len() < long.line.to_string().len(),
            "the unscrolled view is still clipped to the pane"
        );
    }

    #[test]
    fn the_pane_says_whether_it_is_refreshing_or_showing_a_cached_answer() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = overview_app();
        app.pull_request_exact_number = Some(42);
        let render = |app: &mut App, terminal: &mut Terminal<TestBackend>| {
            terminal.draw(|frame| draw(frame, app)).unwrap();
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
        };
        let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

        app.pull_request_conversation_loading = true;
        let refreshing = render(&mut app, &mut terminal);
        assert!(
            refreshing.contains('⟳'),
            "a read in flight shows a reload mark"
        );
        assert!(!refreshing.contains("cached"));

        app.pull_request_conversation_loading = false;
        app.pull_request_from_cache = true;
        app.pull_request_checks_from_cache = true;
        app.pull_request_conversation.from_cache = true;
        let cached = render(&mut app, &mut terminal);
        assert!(
            cached.contains("cached"),
            "an answer served from disk says so rather than pretending to be live"
        );
        assert!(!cached.contains('⟳'));

        app.pull_request_checks_from_cache = false;
        let live_checks = render(&mut app, &mut terminal);
        assert!(
            live_checks.contains("cached"),
            "a freshly read check list does not make the pull request itself live"
        );

        app.pull_request_conversation.from_cache = false;
        let live = render(&mut app, &mut terminal);
        assert!(!live.contains("cached"));
        assert!(!live.contains('⟳'));
    }

    #[test]
    fn a_failed_lookup_stays_readable_after_its_toast_expires() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::PullRequests;
        app.pull_request_exact_number = Some(404);
        app.pull_request_error =
            Some("unable to load pull request: GraphQL: Could not resolve to a PullRequest".into());
        let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(rendered.contains("could not be opened"));
        assert!(rendered.contains("Could not resolve to a PullRequest"));
        assert!(rendered.contains("Press r to try again"));
    }

    #[test]
    fn pull_request_overview_reads_as_a_conversation_beside_its_checks() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = overview_app();
        app.pull_request_conversation = crate::git::github::PullRequestConversation {
            truncated: false,
            from_cache: false,
            entries: vec![
                ConversationEntry {
                    kind: ConversationKind::Opened,
                    actor: "octocat".to_owned(),
                    timestamp: "2026-08-01T09:00:00Z".to_owned(),
                    detail: "feature/rocket into main".to_owned(),
                    body: "## Summary".to_owned(),
                    url: String::new(),
                    reference: String::new(),
                    context: String::new(),
                },
                ConversationEntry {
                    kind: ConversationKind::ForcePush,
                    actor: "octocat".to_owned(),
                    timestamp: "2026-08-02T09:10:00Z".to_owned(),
                    detail: String::new(),
                    body: String::new(),
                    url: String::new(),
                    reference: "deadbeefcafe".to_owned(),
                    context: String::new(),
                },
                ConversationEntry {
                    kind: ConversationKind::ReviewComment,
                    actor: "reviewer".to_owned(),
                    timestamp: "2026-08-02T09:30:00Z".to_owned(),
                    detail: "src/main.rs:42".to_owned(),
                    body: "Extract this into a helper".to_owned(),
                    url: String::new(),
                    reference: String::new(),
                    context: "@@ -1 +1 @@\n-old\n+new".to_owned(),
                },
            ],
        };
        let mut terminal = Terminal::new(TestBackend::new(150, 40)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(rendered.contains("Conversation"));
        assert!(rendered.contains("Format, lint, and test"));
        assert!(rendered.contains("#42"));
        assert!(rendered.contains("acme/widget:main"));
        assert!(rendered.contains(&format_local_timestamp("2026-08-01T09:00:00Z")));
        assert!(rendered.contains("Description"));
        assert!(
            rendered.contains("Launch safely"),
            "the body renders as prose, not as raw Markdown"
        );
        assert!(rendered.contains("cargo test"), "fenced code survives");
        assert!(rendered.contains("opened this pull request"));
        assert!(rendered.contains("force-pushed to deadbee"));
        assert!(rendered.contains("commented on src/main.rs:42"));
        assert!(rendered.contains("Extract this into a helper"));
        assert!(
            !rendered.contains("## Summary"),
            "the opening post never repeats the description above it"
        );
    }

    #[test]
    fn selecting_a_check_shows_its_steps_and_opens_the_failure() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = overview_app();
        app.pull_request_check_cursor = Some(0);
        app.pull_request_step_cursor = 2;
        app.expanded_check_steps.insert(2);
        app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
            truncated: false,
            unavailable: None,
            log_pending: false,
            loose_lines: vec![CheckLogLine {
                timestamp: "2026-08-02T10:02:31Z".to_owned(),
                text: "Cleaning up runner".to_owned(),
                severity: CheckLogSeverity::Normal,
            }],
            steps: vec![
                CheckStep {
                    number: 1,
                    name: "Set up job".to_owned(),
                    status: PullRequestCheckStatus::Passed,
                    conclusion: "success".to_owned(),
                    started_at: "2026-08-02T10:00:00Z".to_owned(),
                    completed_at: "2026-08-02T10:00:02Z".to_owned(),
                    lines: vec![CheckLogLine {
                        timestamp: "2026-08-02T10:00:01Z".to_owned(),
                        text: "hidden while folded".to_owned(),
                        severity: CheckLogSeverity::Normal,
                    }],
                },
                CheckStep {
                    number: 2,
                    name: "Run cargo test".to_owned(),
                    status: PullRequestCheckStatus::Failed,
                    conclusion: "failure".to_owned(),
                    started_at: "2026-08-02T10:00:02Z".to_owned(),
                    completed_at: "2026-08-02T10:02:30Z".to_owned(),
                    lines: vec![CheckLogLine {
                        timestamp: "2026-08-02T10:02:29Z".to_owned(),
                        text: "test tests::rockets ... FAILED".to_owned(),
                        severity: CheckLogSeverity::Error,
                    }],
                },
            ],
        });
        let mut terminal = Terminal::new(TestBackend::new(150, 40)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(rendered.contains("2 steps"));
        assert!(rendered.contains("Set up job"));
        assert!(rendered.contains("Run cargo test"));
        assert!(
            rendered.contains("2m 28s"),
            "a step reports how long it ran"
        );
        assert!(rendered.contains("test tests::rockets ... FAILED"));
        assert!(
            !rendered.contains("hidden while folded"),
            "a folded step keeps its output out of the way"
        );
        assert!(rendered.contains("Runner output"));
        assert!(rendered.contains("Cleaning up runner"));
        assert!(
            !app.geometry.content_step_hits.is_empty(),
            "step rows stay clickable"
        );

        let rows = check_run_rows(&app, 40, &Theme::default());
        let log = rows
            .iter()
            .find(|row| row.line.to_string().contains("rockets"))
            .expect("the expanded step's output is rendered");
        assert!(log.wide, "a log line scrolls instead of being truncated");
        assert!(
            log.line
                .to_string()
                .ends_with("test tests::rockets ... FAILED"),
            "the line keeps its full text even in a pane far narrower than it"
        );
    }

    #[test]
    fn pull_request_loading_renders_on_demand_progress_and_skeletons() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::PullRequests;
        app.pull_request_loading = true;
        app.pull_request_exact_number = Some(42);
        app.pull_request_lookup = crate::app::TextBuffer::new("42");
        app.pull_request_progress = Some(crate::git::github::PullRequestProgress::FetchingHead);
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        let bottom = (24..27)
            .flat_map(|row| (0..42).map(move |column| buffer[(column, row)].symbol()))
            .collect::<String>();

        assert!(rendered.contains("50%"));
        assert!(rendered.contains("Fetching the source commit"));
        assert!(rendered.contains('█'));
        assert!(bottom.contains("auto-detect"));
        assert!(bottom.contains("PR #"));
    }

    #[test]
    fn file_header_right_aligns_colored_line_counts() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let header = test_file_header("src/main.rs", 12, 3);
        let theme = Theme::default();
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new("/tmp/repo", "repo");
        app.document.lines = vec![header.clone()];
        terminal
            .draw(|frame| draw_file_header(frame, frame.area(), &header, &app, &theme))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        let addition_column = rendered
            .chars()
            .position(|character| character == '+')
            .unwrap();
        let deletion_column = rendered
            .chars()
            .position(|character| character == '-')
            .unwrap();

        assert!(rendered.ends_with("+12 -3 ┐"));
        assert!(!rendered.contains('⌄'));
        assert!(!rendered.contains('›'));
        assert_eq!(buffer[(cells(addition_column), 0)].fg, theme.added);
        assert_eq!(buffer[(cells(deletion_column), 0)].fg, theme.removed);
    }

    #[test]
    fn skips_intraline_work_for_very_long_rows() {
        let old = test_line(
            DiffLineKind::Removed,
            &"a".repeat(MAX_INTRALINE_SOURCE_BYTES + 1),
        );
        let new = test_line(
            DiffLineKind::Added,
            &"b".repeat(MAX_INTRALINE_SOURCE_BYTES + 1),
        );

        assert_eq!(
            paired_intraline_emphasis(Some(&old), Some(&new)),
            (None, None)
        );
    }

    #[test]
    fn document_details_participate_in_vertical_scroll() {
        assert_eq!(commit_details_row_count(30), 7);
        assert_eq!(commit_details_row_count(8), 5);
        assert_eq!(commit_details_row_count(3), 0);
        assert_eq!(pull_request_details_row_count(30), 12);
        assert_eq!(pull_request_details_row_count(9), 6);
        assert_eq!(pull_request_details_row_count(3), 0);
    }

    #[test]
    fn pull_request_description_preview_is_bounded_and_marks_truncation() {
        let lines =
            description_preview_lines("one two three four five six seven eight nine ten", 10, 3);
        assert_eq!(lines.len(), 3);
        assert!(lines.last().unwrap().ends_with('…'));
        assert!(lines.iter().all(|line| line.width() <= 10));
    }

    #[test]
    fn pull_request_description_preview_cleans_common_markdown() {
        let lines = description_preview_lines(
            "## Summary\n- Add a **Pull Requests** tab\n- Cache raw `gh` metadata",
            24,
            3,
        );
        let rendered = lines.join(" ");

        assert!(rendered.contains("Add a Pull Requests tab"));
        assert!(rendered.contains("•"));
        assert!(!rendered.contains("Summary"));
        assert!(!rendered.contains('#'));
        assert!(!rendered.contains('*'));
        assert!(!rendered.contains('`'));
        assert!(lines.iter().all(|line| line.width() <= 24));
    }

    #[test]
    fn formats_zero_one_and_multiple_remote_aliases() {
        assert_eq!(remote_suffix(&[]), "");
        assert_eq!(remote_suffix(&["origin".to_owned()]), "  ·  remote origin");
        assert_eq!(
            remote_suffix(&["origin".to_owned(), "upstream".to_owned()]),
            "  ·  remotes origin, upstream"
        );
    }

    #[test]
    fn collapse_all_keeps_only_selectable_file_headers() {
        let document = DiffDocument {
            title: String::new(),
            truncated: false,
            commit_details: None,
            pull_request_details: None,
            lines: vec![
                test_file_header("one.rs", 1, 0),
                test_line(DiffLineKind::HunkHeader, "@@ -0,0 +1 @@"),
                test_line(DiffLineKind::Added, "one"),
                test_line(DiffLineKind::FileFooter, ""),
                test_file_header("two.rs", 1, 0),
                test_line(DiffLineKind::Added, "two"),
                test_line(DiffLineKind::FileFooter, ""),
            ],
        };

        let mut app = App::new("/tmp/repo", "repo");
        app.document = document.clone();
        app.files_collapsed = true;
        assert_eq!(unified_row_indices(&document, &app), vec![0, 4]);
        assert_eq!(side_by_side_rows(&document, &app).len(), 2);
    }

    #[test]
    fn computes_vscode_style_intraline_changed_ranges() {
        assert_eq!(
            changed_ranges("const oldValue = 1;", "const newValue = 2;"),
            (Some(6..18), Some(6..18))
        );
        assert_eq!(changed_ranges("same", "same"), (None, None));
        assert_eq!(
            changed_ranges("prefix old suffix", "prefix new suffix"),
            (Some(7..10), Some(7..10))
        );
    }

    fn test_file_header(path: &str, additions: usize, deletions: usize) -> DiffLine {
        DiffLine {
            kind: DiffLineKind::FileHeader,
            old_line: None,
            new_line: None,
            spans: vec![
                HighlightSpan {
                    text: path.to_owned(),
                    foreground: None,
                    bold: false,
                    italic: false,
                },
                HighlightSpan {
                    text: format!("+{additions}"),
                    foreground: None,
                    bold: false,
                    italic: false,
                },
                HighlightSpan {
                    text: format!("-{deletions}"),
                    foreground: None,
                    bold: false,
                    italic: false,
                },
            ],
        }
    }

    fn test_line(kind: DiffLineKind, text: &str) -> DiffLine {
        DiffLine {
            kind,
            old_line: None,
            new_line: None,
            spans: vec![HighlightSpan {
                text: text.to_owned(),
                foreground: None,
                bold: false,
                italic: false,
            }],
        }
    }
}
