use std::path::PathBuf;

use super::*;
use crate::git::status::{Change, ChangeArea, ChangeStatus};

#[test]
fn warming_logs_never_shares_a_lane_with_the_reads_a_reader_waits_on() {
    let pull_request = || Box::new(PullRequest::default());
    let warm = WorkerCommand::PrefetchCheckRunLogs {
        generation: 0,
        pull_request: pull_request(),
        checks: Vec::new(),
    };
    let awaited = [
        WorkerCommand::LoadPullRequestChecks {
            generation: 1,
            pull_request: pull_request(),
            refresh: true,
        },
        WorkerCommand::LoadPullRequestConversation {
            generation: 1,
            pull_request: pull_request(),
        },
        WorkerCommand::LookupPullRequest {
            generation: 1,
            repositories: Vec::new(),
            repository: None,
            number: 1,
            refresh: true,
        },
    ];
    for command in awaited {
        assert_ne!(
            worker_lane(&command),
            worker_lane(&warm),
            "{command:?} must not queue behind the log warm-up"
        );
    }
}

#[test]
fn mailbox_coalesces_previews_and_refreshes() {
    let mut mailbox = Mailbox::default();
    mailbox.push(WorkerCommand::Refresh { generation: 1 });
    mailbox.push(WorkerCommand::Refresh { generation: 2 });
    mailbox.push(WorkerCommand::PrepareLocalDiff {
        generation: 1,
        request: Box::new(LocalDiffRequest::Changes {
            changes: vec![Change {
                path: PathBuf::from("old.rs"),
                original_path: None,
                area: ChangeArea::Unstaged,
                status: ChangeStatus::Modified,
            }],
            version: 1,
            expanded: false,
        }),
    });
    mailbox.push(WorkerCommand::PrepareLocalDiff {
        generation: 2,
        request: Box::new(LocalDiffRequest::Changes {
            changes: vec![Change {
                path: PathBuf::from("new.rs"),
                original_path: None,
                area: ChangeArea::Unstaged,
                status: ChangeStatus::Modified,
            }],
            version: 2,
            expanded: true,
        }),
    });

    assert!(matches!(
        mailbox.pop(),
        Some(WorkerCommand::PrepareLocalDiff { generation: 2, .. })
    ));
    assert!(matches!(
        mailbox.pop(),
        Some(WorkerCommand::Refresh { generation: 2 })
    ));
    assert!(mailbox.pop().is_none());
}

#[test]
fn mailbox_keeps_worktree_reads_off_the_branch_slot() {
    let mut mailbox = Mailbox::default();
    mailbox.push(WorkerCommand::LoadStashes { generation: 1 });
    mailbox.push(WorkerCommand::LoadWorktrees { generation: 2 });
    assert!(matches!(
        mailbox.pop(),
        Some(WorkerCommand::LoadStashes { generation: 1 })
    ));
    assert!(matches!(
        mailbox.pop(),
        Some(WorkerCommand::LoadWorktrees { generation: 2 })
    ));
}

#[test]
fn mailbox_keeps_only_the_latest_explicit_pull_request_lookup() {
    let mut mailbox = Mailbox::default();
    for (generation, number) in [(1, 41), (2, 42)] {
        mailbox.push(WorkerCommand::LookupPullRequest {
            generation,
            repositories: Vec::new(),
            repository: None,
            number,
            refresh: false,
        });
    }
    mailbox.push(WorkerCommand::LoadHistory {
        generation: 1,
        revision: "HEAD".to_owned(),
        skip: 0,
        limit: 300,
    });

    assert!(matches!(
        mailbox.pop(),
        Some(WorkerCommand::LookupPullRequest {
            generation: 2,
            number: 42,
            ..
        })
    ));
    assert!(matches!(
        mailbox.pop(),
        Some(WorkerCommand::LoadHistory { generation: 1, .. })
    ));
    assert!(mailbox.pop().is_none());
}

#[test]
fn local_previews_are_routed_away_from_slow_metadata_and_pr_work() {
    let local_preview = WorkerCommand::PrepareLocalDiff {
        generation: 2,
        request: Box::new(LocalDiffRequest::Changes {
            changes: Vec::new(),
            version: 0,
            expanded: false,
        }),
    };
    assert_eq!(worker_lane(&local_preview), WorkerLane::LocalPreview);

    let request = PullRequest {
        number: 1,
        title: String::new(),
        description: String::new(),
        author: String::new(),
        state: "OPEN".to_owned(),
        is_draft: false,
        created_at: String::new(),
        updated_at: String::new(),
        url: String::new(),
        base_ref: "main".to_owned(),
        base_oid: String::new(),
        head_ref: "topic".to_owned(),
        head_oid: String::new(),
        base_repository: super::super::github::GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: Vec::new(),
        },
        head_repository: None,
        head_remotes: Vec::new(),
        is_cross_repository: false,
        additions: 0,
        deletions: 0,
        changed_files: 0,
    };
    let pr_preview = WorkerCommand::PreparePullRequest {
        generation: 3,
        pull_request: Box::new(request.clone()),
    };
    assert_eq!(worker_lane(&pr_preview), WorkerLane::PullRequestPreview);
    assert_eq!(
        worker_lane(&WorkerCommand::LoadPullRequestConversation {
            generation: 4,
            pull_request: Box::new(request.clone()),
        }),
        WorkerLane::Conversation
    );
    assert_eq!(
        worker_lane(&WorkerCommand::Refresh { generation: 5 }),
        WorkerLane::Background
    );
    assert_eq!(
        worker_lane(&WorkerCommand::LookupPullRequest {
            generation: 6,
            repositories: Vec::new(),
            repository: None,
            number: 42,
            refresh: false,
        }),
        WorkerLane::GitHubMetadata
    );
    assert_eq!(
        worker_lane(&WorkerCommand::LoadPullRequestChecks {
            generation: 7,
            pull_request: Box::new(request),
            refresh: false,
        }),
        WorkerLane::GitHubMetadata
    );
}

#[test]
fn background_prefetch_never_displaces_the_preview_a_reader_is_waiting_for() {
    let mut mailbox = Mailbox::default();
    mailbox.push(WorkerCommand::LoadPullRequestFileBatch {
        workspace_generation: 1,
        paths: vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")],
    });
    mailbox.push(WorkerCommand::LoadPullRequestFile {
        generation: 5,
        workspace_generation: 1,
        path: PathBuf::from("selected.rs"),
    });
    mailbox.push(WorkerCommand::LoadPullRequestFileBatch {
        workspace_generation: 1,
        paths: vec![PathBuf::from("c.rs")],
    });

    assert!(matches!(
        mailbox.pop(),
        Some(WorkerCommand::LoadPullRequestFile { generation: 5, .. })
    ));
    assert!(
        matches!(
            mailbox.pop(),
            Some(WorkerCommand::LoadPullRequestFileBatch { paths, .. })
                if paths == vec![PathBuf::from("c.rs")]
        ),
        "only the newest background batch survives, and it runs after the preview"
    );
    assert!(mailbox.pop().is_none());
}

#[test]
fn mailbox_prioritizes_user_operations() {
    let mut mailbox = Mailbox::default();
    mailbox.push(WorkerCommand::LoadHistory {
        generation: 1,
        revision: "HEAD".to_owned(),
        skip: 0,
        limit: 300,
    });
    mailbox.push(WorkerCommand::Operate {
        id: 7,
        operation: GitOperation::Fetch,
    });

    assert!(matches!(
        mailbox.pop(),
        Some(WorkerCommand::Operate { id: 7, .. })
    ));
}
