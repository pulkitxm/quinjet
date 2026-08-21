
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
