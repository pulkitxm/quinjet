#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn stack_member_rows(
    app: &App,
    width: usize,
    theme: &Theme,
) -> (Vec<ContentRow>, Vec<ContentLink>) {
    match app.stack_inspector.section {
        StackMemberSection::Files => (Vec::new(), Vec::new()),
        StackMemberSection::Summary => stack_summary_rows(app, width, theme),
        StackMemberSection::Conversation => stack_conversation_rows(app, width, theme),
        StackMemberSection::Checks => stack_check_rows(app, theme),
        StackMemberSection::Commits => stack_commit_rows(app, theme),
    }
}

fn stack_summary_rows(
    app: &App,
    width: usize,
    theme: &Theme,
) -> (Vec<ContentRow>, Vec<ContentLink>) {
    let mut rows = Vec::new();
    let mut links = Vec::new();
    let pull_request = app
        .stack_inspector
        .selected_pull_request
        .as_ref()
        .or(app.stack_inspector.selected_locator.as_ref());
    let Some(pull_request) = pull_request else {
        rows.push(ContentRow::text(
            " Loading stack member metadata…",
            Style::default().fg(theme.muted),
        ));
        return (rows, links);
    };
    let review = if pull_request.action_state.review_decision.is_empty() {
        "NO DECISION"
    } else {
        pull_request.action_state.review_decision.as_str()
    };
    rows.push(ContentRow::wide(Line::from(vec![
        Span::styled(" REVIEW ", Style::default().fg(theme.muted)),
        Span::styled(
            review.to_owned(),
            Style::default().fg(if review == "APPROVED" {
                theme.success
            } else if review == "CHANGES_REQUESTED" {
                theme.error
            } else {
                theme.text
            }),
        ),
        Span::styled("   CONVERSATION ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{} events", app.stack_inspector.conversation.entries.len()),
            Style::default().fg(theme.accent),
        ),
        Span::styled("   CHECKS ", Style::default().fg(theme.muted)),
        Span::styled(
            stack_check_summary(app),
            stack_check_summary_style(app, theme),
        ),
    ])));
    rows.push(ContentRow::blank());
    rows.push(ContentRow::plain(section_rule("Description", width, theme)));
    if let Some(error) = app.stack_inspector.selected_error.as_deref() {
        rows.push(ContentRow::text(
            format!(" {error}"),
            Style::default().fg(theme.error),
        ));
    }
    if pull_request.description.trim().is_empty()
        && (app.stack_inspector.selected_error.is_none()
            || app.stack_inspector.selected_pull_request.is_some())
    {
        rows.push(ContentRow::text(
            if app.stack_inspector.selected_loading {
                " Loading the description…"
            } else {
                " No description provided"
            },
            Style::default().fg(theme.muted),
        ));
    } else {
        push_prose(&mut rows, &pull_request.description, "", width, theme);
    }
    rows.push(ContentRow::blank());
    rows.push(ContentRow::plain(section_rule("Details", width, theme)));
    let author_row = rows.len();
    rows.push(ContentRow::wide(detail_line(
        "Author",
        format!("@{}", pull_request.author),
        theme,
    )));
    if let Some(target) = app.account_open_target(&pull_request.author) {
        links.push(ContentLink {
            row: author_row,
            start: DETAIL_LABEL_WIDTH,
            width: pull_request.author.width().saturating_add(1),
            target,
        });
    }
    rows.push(ContentRow::wide(detail_line(
        "Branch",
        format!("{} -> {}", pull_request.base_ref, pull_request.head_ref),
        theme,
    )));
    rows.push(ContentRow::wide(detail_line(
        "Changes",
        format!(
            "{} files · +{} -{}",
            pull_request.changed_files, pull_request.additions, pull_request.deletions
        ),
        theme,
    )));
    rows.push(ContentRow::wide(detail_line(
        "Updated",
        format_relative_timestamp(&pull_request.updated_at),
        theme,
    )));
    rows.push(ContentRow::wide(detail_line(
        "Stack role",
        stack_member_role(app),
        theme,
    )));
    let url_row = rows.len();
    rows.push(ContentRow::wide(link_detail_line(
        "URL",
        pull_request.url.clone(),
        theme,
    )));
    if !pull_request.url.is_empty() {
        links.push(ContentLink {
            row: url_row,
            start: DETAIL_LABEL_WIDTH,
            width: pull_request.url.width(),
            target: OpenTarget::Browser(pull_request.url.clone()),
        });
    }
    (rows, links)
}

fn stack_conversation_rows(
    app: &App,
    width: usize,
    theme: &Theme,
) -> (Vec<ContentRow>, Vec<ContentLink>) {
    let conversation = &app.stack_inspector.conversation;
    let mut rows = vec![ContentRow::plain(section_rule(
        &format!("{} conversation events", conversation.entries.len()),
        width,
        theme,
    ))];
    let mut links = Vec::new();
    if let Some(error) = app.stack_inspector.conversation_error.as_deref() {
        rows.push(ContentRow::text(
            format!(" {error}"),
            Style::default().fg(theme.error),
        ));
    }
    if conversation.entries.is_empty() {
        if app.stack_inspector.conversation_error.is_none() {
            rows.push(ContentRow::text(
                if app.stack_inspector.conversation_loading {
                    " Loading the conversation…"
                } else {
                    " No activity yet"
                },
                Style::default().fg(theme.muted),
            ));
        }
        return (rows, links);
    }
    if conversation.truncated {
        rows.push(ContentRow::text(
            " Older activity was omitted to keep this view bounded",
            Style::default().fg(theme.muted),
        ));
    }
    for entry in &conversation.entries {
        rows.push(ContentRow::blank());
        let start = rows.len();
        push_conversation_entry(&mut rows, entry, width, app, theme);
        let actor = format!("@{}", entry.actor);
        if let Some((row, start, width)) = find_conversation_actor_link(&rows, start, &actor)
            && let Some(target) = app.account_open_target(&entry.actor)
        {
            links.push(ContentLink {
                row,
                start,
                width,
                target,
            });
        }
        if !entry.url.is_empty() {
            let action = rows.get(start).and_then(|content| {
                let span = content.line.spans.get(2)?;
                let start = content.line.spans.iter().take(2).map(Span::width).sum();
                Some((start, span.width()))
            });
            if let Some((action_start, action_width)) = action {
                links.push(ContentLink {
                    row: start,
                    start: action_start,
                    width: action_width,
                    target: OpenTarget::Browser(entry.url.clone()),
                });
            }
        }
    }
    (rows, links)
}

fn stack_check_rows(app: &App, theme: &Theme) -> (Vec<ContentRow>, Vec<ContentLink>) {
    let checks = app.stack_inspector.selected_checks();
    let mut rows = vec![ContentRow::wide(Line::from(vec![
        Span::styled(" CI ", Style::default().fg(theme.muted)),
        Span::styled(
            stack_check_summary(app),
            stack_check_summary_style(app, theme),
        ),
        Span::styled(
            app.stack_inspector
                .selected_locator
                .as_ref()
                .map_or_else(String::new, |pull_request| {
                    format!(" · head {}", short_oid(&pull_request.head_oid))
                }),
            Style::default().fg(theme.muted),
        ),
    ]))];
    let mut links = Vec::new();
    if let Some(error) = app.stack_inspector.selected_checks_error() {
        rows.push(ContentRow::text(
            format!(" {error}"),
            Style::default().fg(theme.error),
        ));
    }
    if checks.checks.is_empty() {
        if app.stack_inspector.selected_checks_error().is_none() {
            rows.push(ContentRow::text(
                if app.stack_inspector.selected_checks_loading() {
                    " Loading checks…"
                } else {
                    " No checks reported"
                },
                Style::default().fg(theme.muted),
            ));
        }
        return (rows, links);
    }
    for check in &checks.checks {
        let row = rows.len();
        let label = stack_check_label(check.status);
        let prefix = format!(" {label:<9}");
        let duration = check.duration_label();
        rows.push(ContentRow::wide(Line::from(vec![
            Span::styled(
                prefix.clone(),
                Style::default().fg(stack_check_color(check.status, theme)),
            ),
            Span::styled(
                check.name.clone(),
                if check.link.is_empty() {
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
                } else {
                    Link::style(theme)
                },
            ),
            Span::styled(
                format!(" · {} · {} {duration}", check.workflow, check.description),
                Style::default().fg(theme.muted),
            ),
        ])));
        if !check.link.is_empty() {
            links.push(ContentLink {
                row,
                start: prefix.width(),
                width: check.name.width(),
                target: OpenTarget::Browser(check.link.clone()),
            });
        }
    }
    (rows, links)
}

fn stack_commit_rows(app: &App, theme: &Theme) -> (Vec<ContentRow>, Vec<ContentLink>) {
    let commits = &app.stack_inspector.commits;
    let mut rows = vec![ContentRow::wide(Line::from(vec![
        Span::styled(
            format!(" {} current commits", commits.commits.len()),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " · {}..{}",
                short_oid(&commits.base_oid),
                short_oid(&commits.head_oid)
            ),
            Style::default().fg(theme.muted),
        ),
    ]))];
    let mut links = Vec::new();
    if let Some(error) = app.stack_inspector.commits_error.as_deref() {
        rows.push(ContentRow::text(
            format!(" {error}"),
            Style::default().fg(theme.error),
        ));
    }
    if commits.commits.is_empty() {
        if app.stack_inspector.commits_error.is_none() {
            rows.push(ContentRow::text(
                if app.stack_inspector.commits_loading {
                    " Loading commits…"
                } else {
                    " No commits reported"
                },
                Style::default().fg(theme.muted),
            ));
        }
        return (rows, links);
    }
    for commit in &commits.commits {
        let row = rows.len();
        let prefix = format!(" {} ", commit.abbreviated_oid);
        rows.push(ContentRow::wide(Line::from(vec![
            Span::styled(prefix.clone(), Link::style(theme)),
            Span::styled(
                commit.subject.clone(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " · @{} · {}",
                    commit.author_login.as_deref().unwrap_or(&commit.author),
                    format_relative_timestamp(&commit.authored_at),
                ),
                Style::default().fg(theme.muted),
            ),
        ])));
        if !commit.url.is_empty() {
            links.push(ContentLink {
                row,
                start: 1,
                width: commit
                    .abbreviated_oid
                    .width()
                    .saturating_add(1)
                    .saturating_add(commit.subject.width()),
                target: OpenTarget::Browser(commit.url.clone()),
            });
        }
    }
    if commits.truncated {
        rows.push(ContentRow::text(
            format!(" Showing 500 of {} commits", commits.total_commits),
            Style::default().fg(theme.muted),
        ));
    }
    (rows, links)
}

fn stack_member_role(app: &App) -> String {
    let Some(stack) = app.pull_request_stack.as_ref() else {
        return String::new();
    };
    let position = app.pull_request_stack_cursor.unwrap_or_default();
    if stack
        .tip()
        .is_some_and(|member| member.position == position)
    {
        "TIP · cumulative integration head".to_owned()
    } else {
        format!("MEMBER {position} of {}", stack.size)
    }
}

fn stack_check_summary(app: &App) -> String {
    if app.stack_inspector.selected_checks_error().is_some() {
        return "UNAVAILABLE".to_owned();
    }
    if !app.stack_inspector.selected_checks_loaded() {
        return if app.stack_inspector.selected_checks_loading() {
            "LOADING".to_owned()
        } else {
            "NOT LOADED".to_owned()
        };
    }
    let checks = &app.stack_inspector.selected_checks().checks;
    let count = |status| checks.iter().filter(|check| check.status == status).count();
    format!(
        "{} PASS · {} FAIL · {} RUN · {} STALE · {} SKIP · {} UNKNOWN",
        count(PullRequestCheckStatus::Passed),
        count(PullRequestCheckStatus::Failed),
        count(PullRequestCheckStatus::Pending),
        count(PullRequestCheckStatus::Cancelled),
        count(PullRequestCheckStatus::Skipped),
        count(PullRequestCheckStatus::Unknown),
    )
}

fn stack_check_summary_style(app: &App, theme: &Theme) -> Style {
    let checks = &app.stack_inspector.selected_checks().checks;
    if app.stack_inspector.selected_checks_error().is_some()
        || checks
            .iter()
            .any(|check| check.status == PullRequestCheckStatus::Failed)
    {
        Style::default()
            .fg(theme.error)
            .add_modifier(Modifier::BOLD)
    } else if app.stack_inspector.selected_checks_loading()
        || checks
            .iter()
            .any(|check| check.status == PullRequestCheckStatus::Pending)
    {
        Style::default()
            .fg(theme.modified)
            .add_modifier(Modifier::BOLD)
    } else if checks
        .iter()
        .any(|check| check.status == PullRequestCheckStatus::Cancelled)
    {
        Style::default()
            .fg(theme.conflict)
            .add_modifier(Modifier::BOLD)
    } else if app.stack_inspector.selected_checks_loaded()
        && !checks.is_empty()
        && checks
            .iter()
            .any(|check| check.status == PullRequestCheckStatus::Passed)
        && checks
            .iter()
            .all(|check| check.status != PullRequestCheckStatus::Unknown)
    {
        Style::default()
            .fg(theme.success)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    }
}

const fn stack_check_label(status: PullRequestCheckStatus) -> &'static str {
    match status {
        PullRequestCheckStatus::Passed => "PASS",
        PullRequestCheckStatus::Failed => "FAIL",
        PullRequestCheckStatus::Pending => "RUNNING",
        PullRequestCheckStatus::Skipped => "SKIP",
        PullRequestCheckStatus::Cancelled => "STALE",
        PullRequestCheckStatus::Unknown => "UNKNOWN",
    }
}

const fn stack_check_color(status: PullRequestCheckStatus, theme: &Theme) -> Color {
    match status {
        PullRequestCheckStatus::Passed => theme.success,
        PullRequestCheckStatus::Failed => theme.error,
        PullRequestCheckStatus::Pending => theme.modified,
        PullRequestCheckStatus::Skipped | PullRequestCheckStatus::Unknown => theme.muted,
        PullRequestCheckStatus::Cancelled => theme.conflict,
    }
}
