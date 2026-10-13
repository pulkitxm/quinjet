#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_pr_navigation(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<SidebarHitArea> {
    let has_stack = app.pull_request_stack.is_some();
    let third = area.width.div_euclid(if has_stack { 4 } else { 3 });
    let first = Rect::new(area.x, area.y, third, 1);
    let second = Rect::new(first.right(), area.y, third, 1);
    let third_tab = Rect::new(
        second.right(),
        area.y,
        if has_stack {
            third
        } else {
            area.width.saturating_sub(third.saturating_mul(2))
        },
        1,
    );
    let overview = if app.selected_pr_commit().is_some() {
        "CI"
    } else {
        "PR"
    };
    draw_pull_request_section_tab(
        frame,
        first,
        overview.to_owned(),
        app.pull_request_section == PullRequestSection::Overview,
        theme,
    );
    draw_pull_request_section_tab(
        frame,
        second,
        format!(
            "Files {}{}",
            app.pull_request_total_files,
            app.search_title_suffix()
        ),
        app.pull_request_section == PullRequestSection::Files,
        theme,
    );
    draw_pull_request_section_tab(
        frame,
        third_tab,
        "Commits".to_owned(),
        app.pull_request_section == PullRequestSection::Commits,
        theme,
    );
    let mut hits = vec![
        SidebarHitArea {
            area: first,
            target: SidebarHit::PullRequestOverview,
        },
        SidebarHitArea {
            area: second,
            target: SidebarHit::PullRequestFiles,
        },
        SidebarHitArea {
            area: third_tab,
            target: SidebarHit::PullRequestCommits,
        },
    ];
    if let Some(stack) = &app.pull_request_stack {
        let stack_tab = Rect::new(
            third_tab.right(),
            area.y,
            area.width.saturating_sub(third.saturating_mul(3)),
            1,
        );
        draw_pull_request_section_tab(
            frame,
            stack_tab,
            format!("Stack {}", stack.size),
            app.pull_request_section == PullRequestSection::Stack,
            theme,
        );
        hits.push(SidebarHitArea {
            area: stack_tab,
            target: SidebarHit::PullRequestStack,
        });
    }
    let scoped = app.selected_pr_commit().is_some();
    if scoped && area.height > 1 {
        let scope = Rect::new(area.x, area.y.saturating_add(1), area.width, 1);
        if let Some(commit) = app.selected_pr_commit() {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!(" {}", commit.abbreviated_oid),
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" · [A All commits]", Style::default().fg(theme.muted)),
                ]))
                .style(Style::default().bg(theme.panel_alt)),
                scope,
            );
        }
        hits.push(SidebarHitArea {
            area: scope,
            target: SidebarHit::PullRequestAllCommits,
        });
    }
    let offset = 1 + u16::from(scoped);
    let list = Rect::new(
        area.x,
        area.y.saturating_add(offset),
        area.width,
        area.height.saturating_sub(offset),
    );
    match app.pull_request_section {
        PullRequestSection::Overview => {
            hits.extend(draw_pull_request_check_list(frame, list, app, theme));
        }
        PullRequestSection::Files => {
            hits.extend(draw_pull_request_file_tree(frame, list, app, theme));
        }
        PullRequestSection::Commits => hits.extend(draw_pr_commit_list(frame, list, app, theme)),
        PullRequestSection::Stack => {}
    }
    hits
}

fn draw_pr_commit_list(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<SidebarHitArea> {
    if area.height == 0 {
        return Vec::new();
    }
    let snapshot = &app.pr_commits.snapshot;
    let label = if snapshot.truncated {
        format!(
            " NEWEST {} OF {} COMMITS",
            snapshot.commits.len(),
            snapshot.total_commits
        )
    } else {
        format!(" COMMITS {} · newest first", snapshot.total_commits)
    };
    frame.render_widget(
        Paragraph::new(label).style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let height = area.height.saturating_sub(1);
    if snapshot.commits.is_empty() {
        let message = app.pr_commits.error.clone().unwrap_or_else(|| {
            if app.pr_commits.loading {
                " Loading commits…".to_owned()
            } else {
                " No commits reported".to_owned()
            }
        });
        frame.render_widget(
            Paragraph::new(message)
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(theme.muted)),
            Rect::new(area.x, area.y.saturating_add(1), area.width, height),
        );
        return Vec::new();
    }
    app.sidebar_viewport(
        app.pr_commits.cursor.saturating_mul(2),
        usize::from(height),
        app.pr_commits.snapshot.commits.len().saturating_mul(2),
    );
    let mut hits = Vec::new();
    for offset in 0..usize::from(height) {
        let row_index = app.sidebar_offset.saturating_add(offset);
        let index = row_index.div_euclid(2);
        let Some(commit) = app.pr_commits.snapshot.commits.get(index) else {
            break;
        };
        let selected = index == app.pr_commits.cursor;
        let row = Rect::new(
            area.x,
            area.y.saturating_add(1).saturating_add(cells(offset)),
            area.width,
            1,
        );
        let text = if row_index.is_multiple_of(2) {
            format!(" {} {}", if selected { "›" } else { " " }, commit.subject)
        } else {
            format!("   {} · {}", commit.abbreviated_oid, commit.author)
        };
        frame.render_widget(
            Paragraph::new(truncate_end(&text, usize::from(area.width))).style(
                Style::default()
                    .fg(if row_index.is_multiple_of(2) {
                        theme.text
                    } else {
                        theme.muted
                    })
                    .bg(if selected {
                        theme.selected
                    } else {
                        theme.panel
                    }),
            ),
            row,
        );
        hits.push(SidebarHitArea {
            area: row,
            target: SidebarHit::PullRequestCommit(index),
        });
    }
    hits
}

pub(super) fn commit_ci_rows(app: &App, theme: &Theme) -> Vec<ContentRow> {
    let Some(commit) = app.selected_pr_commit() else {
        return Vec::new();
    };
    let mut rows = vec![
        ContentRow::text(
            format!("{}  {}", commit.abbreviated_oid, commit.subject),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        ContentRow::text(
            format!(
                "{} · {}",
                commit.author,
                format_relative_timestamp(&commit.authored_at)
            ),
            Style::default().fg(theme.muted),
        ),
        ContentRow::blank(),
        ContentRow::plain(check_summary_line(app, theme)),
    ];
    if let Some(error) = &app.pull_request_checks_error {
        rows.push(ContentRow::text(
            error.clone(),
            Style::default().fg(theme.error),
        ));
    }
    rows.push(ContentRow::blank());
    rows.push(ContentRow::text(
        "Select a check in the sidebar to inspect its steps and logs.",
        Style::default().fg(theme.muted),
    ));
    rows
}

pub(super) fn commit_browser_rows(app: &App, theme: &Theme) -> Vec<ContentRow> {
    let Some(commit) = app.pr_commits.snapshot.commits.get(app.pr_commits.cursor) else {
        let message = app.pr_commits.error.clone().unwrap_or_else(|| {
            if app.pr_commits.loading {
                "Loading commits…".to_owned()
            } else {
                "No commits reported".to_owned()
            }
        });
        return vec![ContentRow::text(message, Style::default().fg(theme.muted))];
    };
    vec![
        ContentRow::text(
            commit.subject.clone(),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        ContentRow::blank(),
        ContentRow::plain(detail_line("Commit", commit.oid.clone(), theme)),
        ContentRow::plain(detail_line("Author", commit.author.clone(), theme)),
        ContentRow::plain(detail_line(
            "Authored",
            format_relative_timestamp(&commit.authored_at),
            theme,
        )),
        ContentRow::plain(detail_line(
            "Committed",
            format_relative_timestamp(&commit.committed_at),
            theme,
        )),
        ContentRow::blank(),
        ContentRow::text(
            "Complete pushed history up to the PR head. Enter or click a commit for files and CI checks.",
            Style::default().fg(theme.text),
        ),
        ContentRow::text(
            "Files compare the selected commit with its first parent.",
            Style::default().fg(theme.muted),
        ),
        ContentRow::text(
            "Shift+F: Files · Shift+P: CI · Shift+A: All commits",
            Style::default().fg(theme.muted),
        ),
    ]
}

pub(super) fn commit_person(name: &str, email: &str, timestamp: &str) -> String {
    let date = format_relative_timestamp(timestamp);
    if name.is_empty() {
        return date;
    }
    if email.is_empty() {
        format!("{name}  ·  {date}")
    } else {
        format!("{name} <{email}>  ·  {date}")
    }
}
