#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
pub(super) fn conversation_rows(app: &App, width: usize, theme: &Theme) -> Vec<ContentRow> {
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
        Span::styled(format!("#{}  ", pull_request.number), Link::style(theme)),
        Span::styled(pull_request.title.clone(), Link::style(theme)),
    ])));
    rows.push(ContentRow::wide(Line::from(vec![
        Span::styled(
            format!("{:<DETAIL_LABEL_WIDTH$}", "State"),
            Style::default().fg(theme.muted),
        ),
        Span::styled(format!("{state}  ·  "), Style::default().fg(theme.text)),
        Span::styled(
            format!("@{}", pull_request.author),
            app.account_open_target(&pull_request.author).map_or_else(
                || Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                |_| Link::style(theme),
            ),
        ),
        Span::styled(
            format!(
                "  ·  opened {}  ·  updated {}",
                format_local_timestamp(&pull_request.created_at),
                format_local_timestamp(&pull_request.updated_at)
            ),
            Style::default().fg(theme.text),
        ),
    ])));
    rows.push(ContentRow::wide(Line::from(vec![
        Span::styled(
            format!("{:<DETAIL_LABEL_WIDTH$}", "Source"),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            pull_request.head_label(),
            app.pull_request_head_branch_open_target()
                .map_or_else(|| Style::default().fg(theme.text), |_| Link::style(theme)),
        ),
        Span::styled(
            if pull_request.is_cross_repository {
                "  ·  fork"
            } else {
                ""
            },
            Style::default().fg(theme.text),
        ),
    ])));
    rows.push(ContentRow::wide(Line::from(vec![
        Span::styled(
            format!("{:<DETAIL_LABEL_WIDTH$}", "Destination"),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            pull_request.base_label(),
            app.pull_request_base_branch_open_target()
                .map_or_else(|| Style::default().fg(theme.text), |_| Link::style(theme)),
        ),
    ])));
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
    rows.push(ContentRow::wide(link_detail_line(
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
        push_conversation_entry(&mut rows, entry, width, app, theme);
    }
    rows
}

pub(super) fn push_conversation_entry(
    rows: &mut Vec<ContentRow>,
    entry: &ConversationEntry,
    width: usize,
    app: &App,
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
            app.account_open_target(&entry.actor).map_or_else(
                || Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                |_| Link::style(theme),
            ),
        ),
        Span::styled(
            format!(" {action}"),
            if entry.url.is_empty() {
                Style::default().fg(theme.muted)
            } else {
                Link::style(theme)
            },
        ),
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

pub(super) fn conversation_marker(
    entry: &ConversationEntry,
    theme: &Theme,
) -> (&'static str, Color, String) {
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
            "↑",
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
            ("◆", theme.muted, "restored the source branch".to_owned())
        }
        ConversationKind::BaseRefChanged => (
            "⇄",
            theme.modified,
            "changed the destination branch".to_owned(),
        ),
        ConversationKind::Other => ("·", theme.muted, "updated this pull request".to_owned()),
    }
}
