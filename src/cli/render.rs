use crate::git::diff::{DiffDocument, DiffLineKind};
use crate::git::github::{
    CheckRunLog, CheckStep, ConversationKind, GitHubRepository, PullRequest, PullRequestCheck,
    PullRequestCheckStatus, PullRequestConversation, PullRequestDiffIndex, PullRequestFileStatus,
    unix_now,
};
use crate::git::history::Commit;
use crate::git::status::{ChangeArea, RepoStatus};
use crate::git::{Branch, HistoryBranch, Stash};

fn ahead_label(count: usize) -> String {
    format!(" ahead {count}")
}

fn behind_label(count: usize) -> String {
    format!(" behind {count}")
}

#[derive(Default)]
struct Report(String);

impl Report {
    fn line(&mut self, text: &str) {
        self.0.push_str(text);
        self.0.push('\n');
    }

    fn blank(&mut self) {
        self.0.push('\n');
    }

    const fn empty(&self) -> bool {
        self.0.is_empty()
    }

    fn finish(self) -> String {
        self.0
    }
}

pub(crate) fn status(status: &RepoStatus) -> String {
    let mut out = Report::default();
    let branch = &status.branch;
    let head = if branch.detached {
        format!("HEAD detached at {}", branch.head)
    } else {
        format!("On branch {}", branch.head)
    };
    out.line(&head);
    if let Some(upstream) = &branch.upstream {
        let mut divergence = String::new();
        if branch.ahead > 0 {
            divergence.push_str(&ahead_label(branch.ahead));
        }
        if branch.behind > 0 {
            divergence.push_str(&behind_label(branch.behind));
        }
        out.line(&format!("Tracking {upstream}{divergence}"));
    }
    if status.changes.is_empty() {
        out.line("\nWorking tree clean");
        return out.finish();
    }
    for area in [
        ChangeArea::Conflict,
        ChangeArea::Staged,
        ChangeArea::Unstaged,
    ] {
        let changes: Vec<_> = status
            .changes
            .iter()
            .filter(|change| change.area == area)
            .collect();
        if changes.is_empty() {
            continue;
        }
        out.line(&format!("\n{} ({})", area.label(), changes.len()));
        for change in changes {
            let rename = change
                .original_path
                .as_ref()
                .map(|from| format!(" (from {})", from.display()))
                .unwrap_or_default();
            out.line(&format!(
                "  {:<3} {}{rename}",
                change.status.code(),
                change.display_path()
            ));
        }
    }
    out.finish()
}

pub(crate) fn diff(document: &DiffDocument) -> String {
    let mut out = Report::default();
    for line in &document.lines {
        let text = line.text();
        match line.kind {
            DiffLineKind::FileHeader => {
                if !out.empty() {
                    out.blank();
                }
                out.line(&text);
            }
            DiffLineKind::FileFooter => {}
            DiffLineKind::HunkHeader | DiffLineKind::Meta => {
                out.line(&text);
            }
            DiffLineKind::Added => {
                out.line(&format!("+{text}"));
            }
            DiffLineKind::Removed => {
                out.line(&format!("-{text}"));
            }
            DiffLineKind::Context => {
                out.line(&format!(" {text}"));
            }
        }
    }
    if document.truncated {
        out.line("\n[output reached Quinjet's size cap and was truncated]");
    }
    out.finish()
}

pub(crate) fn history(commits: &[Commit]) -> String {
    let mut out = Report::default();
    for commit in commits {
        let decorations = if commit.decorations.is_empty() {
            String::new()
        } else {
            format!("  ({})", commit.decorations.join(", "))
        };
        out.line(&format!(
            "{}  {}  {:<16}  {}{decorations}",
            commit.short_id,
            commit.relative_date,
            truncate(&commit.author, 16),
            commit.subject
        ));
    }
    out.finish()
}

pub(crate) fn commit(commit: &Commit) -> String {
    let mut out = Report::default();
    out.line(&format!("commit {}", commit.id));
    if commit.parent_ids.len() > 1 {
        out.line(&format!("Merge:  {}", commit.parent_ids.join(" ")));
    }
    out.line(&format!(
        "Author: {} <{}>",
        commit.author, commit.author_email
    ));
    out.line(&format!("Date:   {}", commit.authored_at));
    out.line(&format!("\n    {}\n", commit.subject));
    out.finish()
}

pub(crate) fn branches(branches: &[Branch]) -> String {
    let mut out = Report::default();
    for branch in branches {
        let upstream = branch
            .upstream
            .as_ref()
            .map(|upstream| format!("  -> {upstream}"))
            .unwrap_or_default();
        out.line(&format!(
            "{} {:<28} {:<10} {}{upstream}",
            if branch.current { "*" } else { " " },
            branch.name,
            branch.short_id,
            branch.relative_date
        ));
    }
    out.finish()
}

pub(crate) fn history_branches(branches: &[HistoryBranch]) -> String {
    let mut out = Report::default();
    for branch in branches {
        out.line(&format!(
            "{} {:<40} {:<8} {:<10} {}",
            if branch.current { "*" } else { " " },
            branch.name,
            if branch.remote { "remote" } else { "local" },
            branch.short_id,
            branch.relative_date
        ));
    }
    out.finish()
}

pub(crate) fn stashes(stashes: &[Stash]) -> String {
    let mut out = Report::default();
    for stash in stashes {
        out.line(&format!(
            "{:<12} {:<10} {:<14} on {}: {}",
            stash.reference, stash.short_id, stash.relative_date, stash.branch, stash.message
        ));
    }
    out.finish()
}

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
        pull_request.author, pull_request.created_at, pull_request.updated_at
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
            entry.timestamp
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

const fn file_status_code(status: PullRequestFileStatus) -> &'static str {
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

const fn conversation_action(kind: ConversationKind) -> &'static str {
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

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::git::diff::{DiffLine, HighlightSpan};
    use crate::git::github::{CheckLogLine, CheckLogSeverity, ConversationEntry, PullRequestFile};
    use crate::git::status::{BranchState, Change, ChangeStatus};

    fn span(text: &str) -> Vec<HighlightSpan> {
        vec![HighlightSpan {
            text: text.to_owned(),
            foreground: None,
            bold: false,
            italic: false,
        }]
    }

    fn line(kind: DiffLineKind, text: &str) -> DiffLine {
        DiffLine {
            kind,
            old_line: None,
            new_line: None,
            spans: span(text),
        }
    }

    fn change(path: &str, area: ChangeArea, status: ChangeStatus) -> Change {
        Change {
            path: PathBuf::from(path),
            original_path: None,
            area,
            status,
        }
    }

    fn check(name: &str, status: PullRequestCheckStatus) -> PullRequestCheck {
        PullRequestCheck {
            name: name.to_owned(),
            workflow: "CI".to_owned(),
            state: "COMPLETED".to_owned(),
            status,
            description: String::new(),
            link: "https://github.com/acme/widget/actions/runs/1/job/2".to_owned(),
            started_at: String::new(),
            completed_at: String::new(),
        }
    }

    #[test]
    fn status_groups_changes_the_way_the_changes_view_does() {
        let snapshot = RepoStatus {
            branch: BranchState {
                head: "main".to_owned(),
                oid: Some("abc".to_owned()),
                upstream: Some("origin/main".to_owned()),
                ahead: 2,
                behind: 1,
                detached: false,
            },
            changes: vec![
                change("kept.rs", ChangeArea::Staged, ChangeStatus::Modified),
                change("new.rs", ChangeArea::Unstaged, ChangeStatus::Untracked),
                change("both.rs", ChangeArea::Conflict, ChangeStatus::Conflicted),
            ],
        };

        let text = status(&snapshot);

        assert!(text.starts_with("On branch main\n"), "{text}");
        assert!(
            text.contains("Tracking origin/main ahead 2 behind 1"),
            "{text}"
        );
        let conflict = text
            .find("Merge Changes (1)")
            .expect("conflicts are listed");
        let staged = text
            .find("Staged Changes (1)")
            .expect("staged changes are listed");
        let unstaged = text
            .find("Changes (1)\n  U")
            .expect("unstaged changes are listed");
        assert!(
            conflict < staged && staged < unstaged,
            "the groups keep the order the interface shows them in: {text}"
        );
    }

    #[test]
    fn a_clean_tree_says_so_instead_of_printing_an_empty_list() {
        let text = status(&RepoStatus::default());

        assert!(text.contains("Working tree clean"), "{text}");
    }

    #[test]
    fn a_detached_head_is_named_as_detached() {
        let snapshot = RepoStatus {
            branch: BranchState {
                head: "9f3c1d7e".to_owned(),
                detached: true,
                ..BranchState::default()
            },
            changes: Vec::new(),
        };

        assert!(status(&snapshot).starts_with("HEAD detached at 9f3c1d7e"));
    }

    #[test]
    fn a_diff_carries_the_markers_a_patch_reader_expects() {
        let document = DiffDocument {
            title: "one.rs".to_owned(),
            lines: vec![
                line(DiffLineKind::FileHeader, "one.rs  · modified"),
                line(DiffLineKind::HunkHeader, "@@ -1 +1,2 @@"),
                line(DiffLineKind::Context, "kept"),
                line(DiffLineKind::Removed, "gone"),
                line(DiffLineKind::Added, "fresh"),
                line(DiffLineKind::FileFooter, ""),
            ],
            truncated: false,
            commit_details: None,
            pull_request_details: None,
        };

        let text = diff(&document);

        assert!(text.contains("\n+fresh\n"), "{text}");
        assert!(text.contains("\n-gone\n"), "{text}");
        assert!(text.contains("\n kept\n"), "{text}");
        assert!(text.contains("@@ -1 +1,2 @@"), "{text}");
        assert!(
            !text.contains("\n\n\n"),
            "a footer prints nothing of its own: {text}"
        );
    }

    #[test]
    fn a_truncated_diff_says_that_it_is_incomplete() {
        let document = DiffDocument {
            title: "big.rs".to_owned(),
            lines: vec![line(DiffLineKind::Added, "one")],
            truncated: true,
            commit_details: None,
            pull_request_details: None,
        };

        assert!(
            diff(&document).contains("size cap"),
            "a reader must never mistake a truncated patch for a whole one"
        );
    }

    #[test]
    fn a_commit_reads_like_a_commit() {
        let entry = Commit {
            id: "a".repeat(40),
            short_id: "aaaaaaaa".to_owned(),
            parent_ids: vec!["b".repeat(40), "c".repeat(40)],
            author: "Ada".to_owned(),
            author_email: "ada@example.com".to_owned(),
            authored_at: "2026-08-16T00:00:00Z".to_owned(),
            committer: "Ada".to_owned(),
            committer_email: "ada@example.com".to_owned(),
            committed_at: "2026-08-16T00:00:00Z".to_owned(),
            relative_date: "1 hour ago".to_owned(),
            subject: "feat: land the thing".to_owned(),
            decorations: vec!["HEAD -> main".to_owned()],
        };

        let one = commit(&entry);
        assert!(one.contains("Author: Ada <ada@example.com>"), "{one}");
        assert!(one.contains("Merge:"), "a merge names its parents: {one}");

        let listing = history(std::slice::from_ref(&entry));
        assert!(listing.contains("aaaaaaaa"), "{listing}");
        assert!(listing.contains("(HEAD -> main)"), "{listing}");
    }

    #[test]
    fn a_branch_listing_marks_the_current_one() {
        let listing = branches(&[
            Branch {
                name: "main".to_owned(),
                current: true,
                upstream: Some("origin/main".to_owned()),
                relative_date: "1 hour ago".to_owned(),
                short_id: "aaaaaaaa".to_owned(),
            },
            Branch {
                name: "topic".to_owned(),
                current: false,
                upstream: None,
                relative_date: "2 days ago".to_owned(),
                short_id: "bbbbbbbb".to_owned(),
            },
        ]);

        assert!(listing.starts_with("* main"), "{listing}");
        assert!(listing.contains("-> origin/main"), "{listing}");
        assert!(listing.contains("\n  topic"), "{listing}");
    }

    #[test]
    fn a_history_branch_says_whether_it_is_remote() {
        let listing = history_branches(&[HistoryBranch {
            name: "origin/main".to_owned(),
            reference: "refs/remotes/origin/main".to_owned(),
            current: false,
            remote: true,
            relative_date: "1 hour ago".to_owned(),
            short_id: "aaaaaaaa".to_owned(),
        }]);

        assert!(listing.contains("remote"), "{listing}");
    }

    #[test]
    fn a_stash_listing_carries_its_reference_and_branch() {
        let listing = stashes(&[Stash {
            reference: "stash@{0}".to_owned(),
            message: "work in progress".to_owned(),
            branch: "main".to_owned(),
            relative_date: "1 hour ago".to_owned(),
            short_id: "aaaaaaaa".to_owned(),
        }]);

        assert!(listing.contains("stash@{0}"), "{listing}");
        assert!(listing.contains("on main: work in progress"), "{listing}");
    }

    #[test]
    fn a_repository_listing_separates_a_discovered_remote_from_an_inferred_one() {
        let listing = repositories(
            &[
                GitHubRepository {
                    name_with_owner: "acme/widget".to_owned(),
                    url: "https://github.com/acme/widget".to_owned(),
                    remotes: vec!["origin".to_owned()],
                },
                GitHubRepository {
                    name_with_owner: "acme/fork".to_owned(),
                    url: "https://github.com/acme/fork".to_owned(),
                    remotes: Vec::new(),
                },
            ],
            &["one remote could not be read".to_owned()],
        );

        assert!(listing.contains("remote origin"), "{listing}");
        assert!(listing.contains("inferred"), "{listing}");
        assert!(
            listing.contains("warning: one remote could not be read"),
            "{listing}"
        );
    }

    #[test]
    fn a_draft_pull_request_says_draft_rather_than_its_state() {
        let mut request = PullRequest {
            number: 12,
            title: "Add the thing".to_owned(),
            description: "  ".to_owned(),
            author: "ada".to_owned(),
            state: "OPEN".to_owned(),
            is_draft: true,
            ..PullRequest::default()
        };

        let text = pull_request(&request);
        assert!(text.contains("#12  Add the thing"), "{text}");
        assert!(text.contains("DRAFT"), "{text}");
        assert!(!text.contains("OPEN"), "{text}");

        request.is_draft = false;
        request.description = "why this exists".to_owned();
        let text = pull_request(&request);
        assert!(text.contains("OPEN"), "{text}");
        assert!(text.contains("why this exists"), "{text}");
    }

    #[test]
    fn a_changed_file_listing_shows_its_status_and_counts() {
        let index = PullRequestDiffIndex {
            files: vec![PullRequestFile {
                path: PathBuf::from("src/main.rs"),
                old_path: None,
                status: PullRequestFileStatus::Modified,
                counts: None,
            }],
            total_files: 2,
            truncated: true,
        };

        let text = pull_request_files(&index);
        assert!(text.starts_with("M src/main.rs"), "{text}");
        assert!(
            text.contains("1 of 2 shown"),
            "a bounded listing says so: {text}"
        );
    }

    #[test]
    fn a_check_listing_counts_what_passed_and_what_did_not() {
        let listing = checks(&[
            check("build", PullRequestCheckStatus::Passed),
            check("lint", PullRequestCheckStatus::Failed),
            check("test", PullRequestCheckStatus::Pending),
        ]);

        assert!(
            listing.contains("1 passed, 1 pending, 1 failed"),
            "{listing}"
        );
        assert!(listing.contains("passed"), "{listing}");

        assert!(checks(&[]).contains("No checks reported"));
    }

    #[test]
    fn every_check_state_has_a_glyph_and_a_word() {
        for status in [
            PullRequestCheckStatus::Passed,
            PullRequestCheckStatus::Failed,
            PullRequestCheckStatus::Pending,
            PullRequestCheckStatus::Skipped,
            PullRequestCheckStatus::Cancelled,
            PullRequestCheckStatus::Unknown,
        ] {
            assert!(!check_glyph(status).is_empty(), "{status:?}");
            assert!(!status_word(status).is_empty(), "{status:?}");
        }
    }

    #[test]
    fn a_run_that_publishes_no_log_says_why_instead_of_printing_nothing() {
        let log = CheckRunLog {
            unavailable: Some("third-party does not publish logs".to_owned()),
            ..CheckRunLog::default()
        };

        let text = check_log(&check("third-party", PullRequestCheckStatus::Passed), &log);

        assert!(text.contains("does not publish logs"), "{text}");
    }

    #[test]
    fn a_run_log_prints_its_steps_and_its_loose_output() {
        let log = CheckRunLog {
            steps: vec![CheckStep {
                number: 1,
                name: "Set up job".to_owned(),
                status: PullRequestCheckStatus::Passed,
                conclusion: "success".to_owned(),
                started_at: String::new(),
                completed_at: String::new(),
                lines: vec![CheckLogLine {
                    timestamp: String::new(),
                    text: "runner ready".to_owned(),
                    severity: CheckLogSeverity::Normal,
                }],
            }],
            loose_lines: vec![CheckLogLine {
                timestamp: String::new(),
                text: "teardown".to_owned(),
                severity: CheckLogSeverity::Normal,
            }],
            truncated: true,
            unavailable: None,
            log_pending: true,
        };

        let text = check_log(&check("build", PullRequestCheckStatus::Pending), &log);

        assert!(text.contains("1. Set up job"), "{text}");
        assert!(text.contains("runner ready"), "{text}");
        assert!(text.contains("Runner output"), "{text}");
        assert!(text.contains("teardown"), "{text}");
        assert!(text.contains("Waiting for the runner"), "{text}");
        assert!(text.contains("size cap"), "{text}");
    }

    #[test]
    fn a_conversation_names_the_actor_and_what_they_did() {
        let conversation_value = PullRequestConversation {
            entries: vec![
                ConversationEntry {
                    kind: ConversationKind::Opened,
                    actor: "ada".to_owned(),
                    timestamp: "2026-08-16T00:00:00Z".to_owned(),
                    detail: String::new(),
                    body: "the opening post".to_owned(),
                    url: String::new(),
                    reference: String::new(),
                    context: String::new(),
                },
                ConversationEntry {
                    kind: ConversationKind::ReviewComment,
                    actor: "grace".to_owned(),
                    timestamp: "2026-08-16T01:00:00Z".to_owned(),
                    detail: "src/main.rs:12".to_owned(),
                    body: "this looks wrong".to_owned(),
                    url: String::new(),
                    reference: String::new(),
                    context: "-old\n+new".to_owned(),
                },
            ],
            truncated: true,
            from_cache: false,
        };

        let text = conversation(&conversation_value);

        assert!(text.contains("@ada opened this"), "{text}");
        assert!(
            text.contains("@grace commented on src/main.rs:12"),
            "{text}"
        );
        assert!(
            text.contains("  | -old"),
            "quoted code keeps its gutter: {text}"
        );
        assert!(text.contains("this looks wrong"), "{text}");
        assert!(text.contains("entry cap"), "{text}");
    }

    #[test]
    fn every_timeline_event_has_an_action_phrase() {
        for kind in [
            ConversationKind::Opened,
            ConversationKind::Comment,
            ConversationKind::Review,
            ConversationKind::ReviewComment,
            ConversationKind::Commit,
            ConversationKind::ForcePush,
            ConversationKind::Merged,
            ConversationKind::Closed,
            ConversationKind::Reopened,
            ConversationKind::Labeled,
            ConversationKind::Unlabeled,
            ConversationKind::Renamed,
            ConversationKind::ReadyForReview,
            ConversationKind::ConvertedToDraft,
            ConversationKind::ReviewRequested,
            ConversationKind::ReviewRequestRemoved,
            ConversationKind::Assigned,
            ConversationKind::Unassigned,
            ConversationKind::CrossReferenced,
            ConversationKind::HeadRefDeleted,
            ConversationKind::HeadRefRestored,
            ConversationKind::BaseRefChanged,
            ConversationKind::Other,
        ] {
            assert!(!conversation_action(kind).is_empty(), "{kind:?}");
        }
    }

    #[test]
    fn every_changed_file_status_has_a_code_and_a_label() {
        for status in [
            PullRequestFileStatus::Added,
            PullRequestFileStatus::Modified,
            PullRequestFileStatus::Deleted,
            PullRequestFileStatus::Renamed,
            PullRequestFileStatus::Copied,
            PullRequestFileStatus::TypeChanged,
            PullRequestFileStatus::Unmerged,
            PullRequestFileStatus::Unknown,
        ] {
            assert!(!file_status_code(status).is_empty(), "{status:?}");
        }
        assert_eq!(
            pull_request_file_label(PullRequestFileStatus::Added),
            "added"
        );
        assert_eq!(pull_request_file_label(PullRequestFileStatus::Unknown), "");
    }

    #[test]
    fn a_long_name_is_shortened_rather_than_wrapped() {
        let listing = checks(&[check(&"n".repeat(80), PullRequestCheckStatus::Passed)]);

        assert!(
            listing.contains('…'),
            "an over-long name is elided: {listing}"
        );
    }
}
