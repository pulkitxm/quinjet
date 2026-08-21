use std::path::PathBuf;

use super::*;
use crate::git::github::PullRequestCheckStatus;

#[test]
fn every_worker_command_has_the_expected_lane() {
    let scenarios = lane_scenarios();
    assert_eq!(scenarios.len(), 24);
    for (command, expected) in scenarios {
        assert_eq!(worker_lane(&command), expected, "{command:?}");
    }
}

#[test]
fn every_coalesced_slot_keeps_only_its_latest_command() {
    assert_latest(
        vec![
            WorkerCommand::LoadBranches { generation: 1 },
            WorkerCommand::LoadHistoryBranches { generation: 2 },
            WorkerCommand::LoadStashes { generation: 3 },
        ],
        ("stashes", 3),
    );
    assert_latest(
        vec![
            WorkerCommand::LoadWorktrees { generation: 1 },
            WorkerCommand::LoadRecentProjects { generation: 2 },
        ],
        ("projects", 2),
    );
    assert_latest(
        vec![
            WorkerCommand::Refresh { generation: 1 },
            WorkerCommand::Refresh { generation: 2 },
        ],
        ("refresh", 2),
    );
    assert_latest(
        vec![
            local_diff(1),
            WorkerCommand::LoadLocalDiffFile {
                generation: 2,
                workspace_generation: 20,
                path: PathBuf::from("new.rs"),
            },
        ],
        ("local-file", 2),
    );
    assert_latest(
        vec![
            WorkerCommand::PreparePullRequest {
                generation: 1,
                pull_request: pull_request(),
            },
            WorkerCommand::LoadPullRequestFile {
                generation: 2,
                workspace_generation: 20,
                path: PathBuf::from("new.rs"),
            },
        ],
        ("pr-file", 2),
    );
    assert_latest(vec![history(1), history(2)], ("history", 2));
    assert_latest(
        vec![
            WorkerCommand::LoadGitHubRepositories {
                generation: 1,
                refresh: false,
            },
            WorkerCommand::LoadLocalGitHubRepository,
        ],
        ("local-repository", 0),
    );
    assert_latest(vec![lookup(1), lookup(2)], ("lookup", 2));
    assert_latest(vec![batch(1), batch(2)], ("pr-batch", 2));
    assert_latest(vec![checks(1), checks(2)], ("checks", 2));
    assert_latest(vec![conversation(1), conversation(2)], ("conversation", 2));
    assert_latest(vec![review(1), review(2)], ("review", 2));
    assert_latest(vec![check_log(1), check_log(2)], ("check-log", 2));
    assert_latest(vec![warm(10), warm(20)], ("warm", 20));
}

#[test]
fn local_pull_request_and_review_mutations_remain_fifo() {
    let mut mailbox = Mailbox::default();
    mailbox.push(WorkerCommand::Operate {
        id: 1,
        operation: GitOperation::Fetch,
    });
    mailbox.push(WorkerCommand::OperatePullRequest {
        id: 2,
        pull_request: pull_request(),
        operation: PullRequestOperation::SetDraft(true),
    });
    mailbox.push(WorkerCommand::OperatePullRequestReview {
        generation: 3,
        pull_request: pull_request(),
        operation: PullRequestReviewOperation::Discard,
    });
    mailbox.push(WorkerCommand::Operate {
        id: 4,
        operation: GitOperation::Push,
    });

    for expected in [
        ("operate", 1),
        ("operate-pr", 2),
        ("operate-review", 3),
        ("operate", 4),
    ] {
        assert_eq!(mailbox.pop().as_ref().map(identity), Some(expected));
    }
    assert!(mailbox.pop().is_none());
}

#[test]
fn mailbox_pop_order_covers_every_priority_level() {
    let commands = vec![
        WorkerCommand::Operate {
            id: 1,
            operation: GitOperation::Fetch,
        },
        WorkerCommand::LoadBranches { generation: 2 },
        WorkerCommand::LoadWorktrees { generation: 3 },
        local_diff(4),
        WorkerCommand::LoadGitHubRepositories {
            generation: 5,
            refresh: false,
        },
        lookup(6),
        WorkerCommand::Refresh { generation: 7 },
        check_log(8),
        checks(9),
        conversation(10),
        review(11),
        history(12),
        batch(13),
        warm(14),
    ];
    let expected = [
        ("operate", 1),
        ("branches", 2),
        ("worktrees", 3),
        ("local-diff", 4),
        ("repositories", 5),
        ("lookup", 6),
        ("refresh", 7),
        ("check-log", 8),
        ("checks", 9),
        ("conversation", 10),
        ("review", 11),
        ("history", 12),
        ("pr-batch", 13),
        ("warm", 14),
    ];
    let mut mailbox = Mailbox::default();
    for command in commands.into_iter().rev() {
        mailbox.push(command);
    }

    for expected in expected {
        assert_eq!(mailbox.pop().as_ref().map(identity), Some(expected));
    }
    assert!(mailbox.pop().is_none());
}

#[test]
fn shutdown_dominates_queued_work_and_rejects_later_sends() {
    let mailbox = new_mailbox();
    {
        let mut state = mailbox.state.lock().unwrap();
        state.push(WorkerCommand::Refresh { generation: 1 });
        state.push(WorkerCommand::Operate {
            id: 2,
            operation: GitOperation::Fetch,
        });
        state.push(WorkerCommand::Shutdown);
    }
    assert!(next_command(&mailbox).is_none());

    let repository = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let worker = GitWorker::start(repository);
    assert!(worker.send(WorkerCommand::Shutdown));
    assert!(!worker.send(WorkerCommand::Refresh { generation: 3 }));
}

fn lane_scenarios() -> Vec<(WorkerCommand, WorkerLane)> {
    vec![
        (
            WorkerCommand::Refresh { generation: 1 },
            WorkerLane::Background,
        ),
        (local_diff(2), WorkerLane::LocalPreview),
        (
            WorkerCommand::LoadLocalDiffFile {
                generation: 3,
                workspace_generation: 2,
                path: PathBuf::from("local.rs"),
            },
            WorkerLane::LocalPreview,
        ),
        (history(4), WorkerLane::Background),
        (
            WorkerCommand::LoadGitHubRepositories {
                generation: 5,
                refresh: false,
            },
            WorkerLane::GitHubMetadata,
        ),
        (
            WorkerCommand::LoadLocalGitHubRepository,
            WorkerLane::Background,
        ),
        (lookup(6), WorkerLane::GitHubMetadata),
        (
            WorkerCommand::PreparePullRequest {
                generation: 7,
                pull_request: pull_request(),
            },
            WorkerLane::PullRequestPreview,
        ),
        (
            WorkerCommand::LoadPullRequestFile {
                generation: 8,
                workspace_generation: 7,
                path: PathBuf::from("pull.rs"),
            },
            WorkerLane::PullRequestPreview,
        ),
        (batch(9), WorkerLane::PullRequestPreview),
        (checks(10), WorkerLane::GitHubMetadata),
        (conversation(11), WorkerLane::Conversation),
        (review(12), WorkerLane::Review),
        (
            WorkerCommand::OperatePullRequestReview {
                generation: 13,
                pull_request: pull_request(),
                operation: PullRequestReviewOperation::Discard,
            },
            WorkerLane::Review,
        ),
        (check_log(14), WorkerLane::GitHubMetadata),
        (warm(15), WorkerLane::Warm),
        (
            WorkerCommand::LoadBranches { generation: 16 },
            WorkerLane::Background,
        ),
        (
            WorkerCommand::LoadHistoryBranches { generation: 17 },
            WorkerLane::Background,
        ),
        (
            WorkerCommand::LoadStashes { generation: 18 },
            WorkerLane::Background,
        ),
        (
            WorkerCommand::LoadWorktrees { generation: 19 },
            WorkerLane::Background,
        ),
        (
            WorkerCommand::LoadRecentProjects { generation: 20 },
            WorkerLane::Background,
        ),
        (
            WorkerCommand::Operate {
                id: 21,
                operation: GitOperation::Fetch,
            },
            WorkerLane::Background,
        ),
        (
            WorkerCommand::OperatePullRequest {
                id: 22,
                pull_request: pull_request(),
                operation: PullRequestOperation::SetDraft(true),
            },
            WorkerLane::Background,
        ),
        (WorkerCommand::Shutdown, WorkerLane::Background),
    ]
}

fn local_diff(generation: u64) -> WorkerCommand {
    WorkerCommand::PrepareLocalDiff {
        generation,
        request: Box::new(LocalDiffRequest::Changes {
            changes: Vec::new(),
            version: generation,
            expanded: false,
        }),
    }
}

fn history(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadHistory {
        generation,
        revision: "HEAD".to_owned(),
        skip: 0,
        limit: 300,
    }
}

fn lookup(generation: u64) -> WorkerCommand {
    WorkerCommand::LookupPullRequest {
        generation,
        repositories: Vec::new(),
        repository: None,
        number: generation,
        refresh: false,
    }
}

fn batch(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadPullRequestFileBatch {
        workspace_generation: generation,
        paths: vec![PathBuf::from(format!("file-{generation}.rs"))],
    }
}

fn checks(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadPullRequestChecks {
        generation,
        pull_request: pull_request(),
        refresh: false,
    }
}

fn conversation(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadPullRequestConversation {
        generation,
        pull_request: pull_request(),
    }
}

fn review(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadPullRequestReview {
        generation,
        pull_request: pull_request(),
    }
}

fn check_log(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadCheckRunLog {
        generation,
        pull_request: pull_request(),
        check: Box::new(PullRequestCheck {
            name: "check".to_owned(),
            workflow: "workflow".to_owned(),
            state: "completed".to_owned(),
            status: PullRequestCheckStatus::Passed,
            description: String::new(),
            link: String::new(),
            started_at: String::new(),
            completed_at: String::new(),
        }),
    }
}

fn warm(generation: u64) -> WorkerCommand {
    WorkerCommand::PrefetchCheckRunLogs {
        generation,
        pull_request: pull_request(),
        checks: Vec::new(),
    }
}

fn pull_request() -> Box<PullRequest> {
    Box::default()
}

fn assert_latest(commands: Vec<WorkerCommand>, expected: (&'static str, u64)) {
    let mut mailbox = Mailbox::default();
    for command in commands {
        mailbox.push(command);
    }
    assert_eq!(mailbox.pop().as_ref().map(identity), Some(expected));
    assert!(mailbox.pop().is_none());
}

fn identity(command: &WorkerCommand) -> (&'static str, u64) {
    match command {
        WorkerCommand::Refresh { generation } => ("refresh", *generation),
        WorkerCommand::PrepareLocalDiff { generation, .. } => ("local-diff", *generation),
        WorkerCommand::LoadLocalDiffFile { generation, .. } => ("local-file", *generation),
        WorkerCommand::LoadHistory { generation, .. } => ("history", *generation),
        WorkerCommand::LoadGitHubRepositories { generation, .. } => ("repositories", *generation),
        WorkerCommand::LoadLocalGitHubRepository => ("local-repository", 0),
        WorkerCommand::LookupPullRequest { generation, .. } => ("lookup", *generation),
        WorkerCommand::PreparePullRequest { generation, .. } => ("prepare-pr", *generation),
        WorkerCommand::LoadPullRequestFile { generation, .. } => ("pr-file", *generation),
        WorkerCommand::LoadPullRequestFileBatch {
            workspace_generation,
            ..
        } => ("pr-batch", *workspace_generation),
        WorkerCommand::LoadPullRequestChecks { generation, .. } => ("checks", *generation),
        WorkerCommand::LoadPullRequestConversation { generation, .. } => {
            ("conversation", *generation)
        }
        WorkerCommand::LoadPullRequestReview { generation, .. } => ("review", *generation),
        WorkerCommand::OperatePullRequestReview { generation, .. } => {
            ("operate-review", *generation)
        }
        WorkerCommand::LoadCheckRunLog { generation, .. } => ("check-log", *generation),
        WorkerCommand::PrefetchCheckRunLogs { generation, .. } => ("warm", *generation),
        WorkerCommand::LoadBranches { generation } => ("branches", *generation),
        WorkerCommand::LoadHistoryBranches { generation } => ("history-branches", *generation),
        WorkerCommand::LoadStashes { generation } => ("stashes", *generation),
        WorkerCommand::LoadWorktrees { generation } => ("worktrees", *generation),
        WorkerCommand::LoadRecentProjects { generation } => ("projects", *generation),
        WorkerCommand::Operate { id, .. } => ("operate", *id),
        WorkerCommand::OperatePullRequest { id, .. } => ("operate-pr", *id),
        WorkerCommand::Shutdown => ("shutdown", 0),
    }
}
