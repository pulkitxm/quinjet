use super::*;

pub(crate) fn repositories(repositories: &[GitHubRepository], warnings: &[String]) -> String {
    let mut out = Report::default();
    for repository in repositories {
        let remotes = if repository.remotes.is_empty() {
            "inferred".to_owned()
        } else {
            format!("remote {}", repository.remotes.join(", "))
        };
        out.line(&format!(
            "{:<40} {:<10} {}",
            repository.display_name(),
            remotes,
            repository.url
        ));
    }
    for warning in warnings {
        out.line(&format!("warning: {warning}"));
    }
    out.finish()
}

pub(crate) fn pull_request(pull_request: &PullRequest) -> String {
    let mut out = Report::default();
    let state = if pull_request.is_draft {
        "DRAFT".to_owned()
    } else {
        pull_request.state.clone()
    };
    out.line(&format!("#{}  {}", pull_request.number, pull_request.title));
    out.line(&format!(
        "{state} · @{} · opened {} · updated {}",
        pull_request.author,
        format_local_timestamp(&pull_request.created_at),
        format_local_timestamp(&pull_request.updated_at)
    ));
    out.line(&format!("Source       {}", pull_request.head_label()));
    out.line(&format!("Destination  {}", pull_request.base_label()));
    out.line(&format!(
        "Changes      {} files, +{} -{}",
        pull_request.changed_files, pull_request.additions, pull_request.deletions
    ));
    out.line(&format!("URL          {}", pull_request.url));
    if !pull_request.description.trim().is_empty() {
        out.line(&format!("\n{}", pull_request.description.trim_end()));
    }
    out.finish()
}

pub(crate) fn pull_request_files(index: &PullRequestDiffIndex) -> String {
    let mut out = Report::default();
    for file in &index.files {
        let counts = file
            .counts
            .map(|counts| {
                if counts.binary {
                    "  binary".to_owned()
                } else {
                    format!("  +{} -{}", counts.additions, counts.deletions)
                }
            })
            .unwrap_or_default();
        let rename = file
            .old_path
            .as_ref()
            .map(|from| format!(" (from {})", from.display()))
            .unwrap_or_default();
        out.line(&format!(
            "{} {}{rename}{counts}",
            file_status_code(file.status),
            file.path.display()
        ));
    }
    if index.truncated {
        out.line(&format!(
            "\n[the changed-file list reached Quinjet's size cap; {} of {} shown]",
            index.files.len(),
            index.total_files
        ));
    }
    out.finish()
}

pub(crate) fn checks(checks: &[PullRequestCheck]) -> String {
    if checks.is_empty() {
        return "No checks reported\n".to_owned();
    }
    let mut out = Report::default();
    for check in checks {
        let duration = check.duration_label();
        let duration = if duration.is_empty() {
            String::new()
        } else {
            format!("  {duration}")
        };
        out.line(&format!(
            "{}  {:<9} {:<44} {}{duration}",
            check_glyph(check.status),
            status_word(check.status),
            truncate(&check.name, 44),
            check.workflow
        ));
    }
    out.line(&format!("\n{}", check_summary(checks)));
    out.finish()
}

pub(crate) fn check_summary(checks: &[PullRequestCheck]) -> String {
    let mut passed = 0;
    let mut pending = 0;
    let mut failed = 0;
    for check in checks {
        match check.status {
            PullRequestCheckStatus::Passed => passed += 1,
            PullRequestCheckStatus::Pending => pending += 1,
            PullRequestCheckStatus::Failed => failed += 1,
            _ => {}
        }
    }
    format!("{passed} passed, {pending} pending, {failed} failed")
}

pub(crate) fn check_log(check: &PullRequestCheck, log: &CheckRunLog) -> String {
    let mut out = Report::default();
    out.line(&format!(
        "{}  {}  ({} · {})",
        check_glyph(check.status),
        check.name,
        check.workflow,
        check.state
    ));
    if !check.link.is_empty() {
        out.line(&check.link);
    }
    if let Some(reason) = &log.unavailable {
        out.line(&format!("\n{reason}"));
        return out.finish();
    }
    if log.log_pending {
        out.line("\nWaiting for the runner to write its first output");
    }
    let now = unix_now();
    for step in &log.steps {
        out.line(&format!("\n{}", step_heading(step, now)));
        for line in &step.lines {
            out.line(&format!("  {}", line.text));
        }
    }
    if !log.loose_lines.is_empty() {
        out.line("\nRunner output");
        for line in &log.loose_lines {
            out.line(&format!("  {}", line.text));
        }
    }
    if log.truncated {
        out.line("\n[the log reached Quinjet's size cap and was truncated]");
    }
    out.finish()
}

pub(crate) fn conversation(conversation: &PullRequestConversation) -> String {
    let mut out = Report::default();
    for entry in &conversation.entries {
        let detail = if entry.detail.is_empty() {
            String::new()
        } else {
            format!(" {}", entry.detail)
        };
        out.line(&format!(
            "\n@{} {}{detail}  ({})",
            entry.actor,
            conversation_action(entry.kind),
            format_local_timestamp(&entry.timestamp)
        ));
        if !entry.context.is_empty() {
            for line in entry.context.lines() {
                out.line(&format!("  | {line}"));
            }
        }
        if entry.kind.has_body() && !entry.body.trim().is_empty() {
            for line in entry.body.trim_end().lines() {
                out.line(&format!("  {line}"));
            }
        }
    }
    if conversation.truncated {
        out.line("\n[the conversation reached Quinjet's entry cap and older entries were dropped]");
    }
    out.finish()
}

fn step_heading(step: &CheckStep, now: i64) -> String {
    let duration = step.duration_label(now);
    let duration = if duration.is_empty() {
        String::new()
    } else {
        format!("  {duration}")
    };
    format!(
        "{}  {}. {}{duration}",
        check_glyph(step.status),
        step.number,
        step.name
    )
}

pub(crate) const fn check_glyph(status: PullRequestCheckStatus) -> &'static str {
    match status {
        PullRequestCheckStatus::Passed => "+",
        PullRequestCheckStatus::Failed => "x",
        PullRequestCheckStatus::Pending => "o",
        PullRequestCheckStatus::Skipped => "-",
        PullRequestCheckStatus::Cancelled => "/",
        PullRequestCheckStatus::Unknown => "?",
    }
}

pub(crate) const fn status_word(status: PullRequestCheckStatus) -> &'static str {
    match status {
        PullRequestCheckStatus::Passed => "passed",
        PullRequestCheckStatus::Failed => "failed",
        PullRequestCheckStatus::Pending => "pending",
        PullRequestCheckStatus::Skipped => "skipped",
        PullRequestCheckStatus::Cancelled => "cancelled",
        PullRequestCheckStatus::Unknown => "unknown",
    }
}

pub(crate) const fn pull_request_file_label(status: PullRequestFileStatus) -> &'static str {
    match status {
        PullRequestFileStatus::Added => "added",
        PullRequestFileStatus::Modified => "modified",
        PullRequestFileStatus::Deleted => "deleted",
        PullRequestFileStatus::Renamed => "renamed",
        PullRequestFileStatus::Copied => "copied",
        PullRequestFileStatus::TypeChanged => "type changed",
        PullRequestFileStatus::Unmerged => "unmerged",
        PullRequestFileStatus::Unknown => "",
    }
}

pub(crate) const fn file_status_code(status: PullRequestFileStatus) -> &'static str {
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

pub(crate) const fn conversation_action(kind: ConversationKind) -> &'static str {
    match kind {
        ConversationKind::Opened => "opened this",
        ConversationKind::Comment => "commented",
        ConversationKind::Review => "reviewed",
        ConversationKind::ReviewComment => "commented on",
        ConversationKind::Commit => "pushed",
        ConversationKind::ForcePush => "force-pushed",
        ConversationKind::Merged => "merged this",
        ConversationKind::Closed => "closed this",
        ConversationKind::Reopened => "reopened this",
        ConversationKind::Labeled => "added the label",
        ConversationKind::Unlabeled => "removed the label",
        ConversationKind::Renamed => "renamed this from",
        ConversationKind::ReadyForReview => "marked this ready for review",
        ConversationKind::ConvertedToDraft => "converted this to a draft",
        ConversationKind::ReviewRequested => "requested a review from",
        ConversationKind::ReviewRequestRemoved => "cancelled the review request for",
        ConversationKind::Assigned => "assigned",
        ConversationKind::Unassigned => "unassigned",
        ConversationKind::CrossReferenced => "referenced this in",
        ConversationKind::HeadRefDeleted => "deleted the head branch",
        ConversationKind::HeadRefRestored => "restored the head branch",
        ConversationKind::BaseRefChanged => "changed the base branch",
        ConversationKind::Other => "acted on this",
    }
}
