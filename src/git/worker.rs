use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use super::diff::DiffDocument;
use super::github::{PullRequest, PullRequestProgress, PullRequestSnapshot};
use super::history::Commit;
use super::status::{Change, RepoStatus};
use super::{Branch, GitOperation, HistoryBranch, Repository, Stash};

#[derive(Debug)]
pub enum WorkerCommand {
    Refresh {
        generation: u64,
    },
    LoadDiff {
        generation: u64,
        changes: Vec<Change>,
        expanded: bool,
    },
    LoadHistory {
        generation: u64,
        revision: String,
        skip: usize,
        limit: usize,
    },
    LoadCommit {
        generation: u64,
        commit: Box<Commit>,
    },
    LoadBranchDiff {
        generation: u64,
        branch: Box<HistoryBranch>,
        expanded: bool,
    },
    LoadStashDiff {
        generation: u64,
        stash: Box<Stash>,
    },
    LoadGitHubRepositories {
        generation: u64,
        refresh: bool,
    },
    LookupPullRequest {
        generation: u64,
        repositories: Vec<super::github::GitHubRepository>,
        repository: Option<Box<super::github::GitHubRepository>>,
        number: u64,
        refresh: bool,
    },
    LoadPullRequest {
        generation: u64,
        pull_request: Box<PullRequest>,
        file_page: usize,
        file_page_size: usize,
    },
    LoadBranches {
        generation: u64,
    },
    LoadHistoryBranches {
        generation: u64,
    },
    LoadStashes {
        generation: u64,
    },
    Operate {
        id: u64,
        operation: GitOperation,
    },
    Shutdown,
}

#[derive(Debug)]
pub enum WorkerEvent {
    Status {
        generation: u64,
        result: Result<RepoStatus, String>,
    },
    Diff {
        generation: u64,
        result: Result<DiffDocument, String>,
    },
    History {
        generation: u64,
        skip: usize,
        result: Result<Vec<Commit>, String>,
    },
    CommitDetail {
        generation: u64,
        result: Result<DiffDocument, String>,
    },
    BranchDiff {
        generation: u64,
        result: Result<DiffDocument, String>,
    },
    StashDiff {
        generation: u64,
        result: Result<DiffDocument, String>,
    },
    GitHubRepositories {
        generation: u64,
        result: Result<(Vec<super::github::GitHubRepository>, Vec<String>), String>,
    },
    PullRequestLookup {
        generation: u64,
        result: Result<PullRequestSnapshot, String>,
    },
    PullRequestProgress {
        generation: u64,
        diff: bool,
        progress: PullRequestProgress,
    },
    PullRequestDiff {
        generation: u64,
        result: Result<DiffDocument, String>,
    },
    Branches {
        generation: u64,
        result: Result<Vec<Branch>, String>,
    },
    HistoryBranches {
        generation: u64,
        result: Result<Vec<HistoryBranch>, String>,
    },
    Stashes {
        generation: u64,
        result: Result<Vec<Stash>, String>,
    },
    OperationFinished {
        id: u64,
        label: String,
        changes_history: bool,
        result: Result<String, String>,
    },
}

#[derive(Default)]
struct Mailbox {
    operations: VecDeque<WorkerCommand>,
    branches: Option<WorkerCommand>,
    refresh: Option<WorkerCommand>,
    preview: Option<WorkerCommand>,
    history: Option<WorkerCommand>,
    pull_request: Option<WorkerCommand>,
    shutdown: bool,
}

impl Mailbox {
    fn push(&mut self, command: WorkerCommand) {
        match command {
            command @ WorkerCommand::Operate { .. } => self.operations.push_back(command),
            command @ (WorkerCommand::LoadBranches { .. }
            | WorkerCommand::LoadHistoryBranches { .. }
            | WorkerCommand::LoadStashes { .. }) => self.branches = Some(command),
            command @ WorkerCommand::Refresh { .. } => self.refresh = Some(command),
            command @ (WorkerCommand::LoadDiff { .. }
            | WorkerCommand::LoadCommit { .. }
            | WorkerCommand::LoadBranchDiff { .. }
            | WorkerCommand::LoadStashDiff { .. }
            | WorkerCommand::LoadPullRequest { .. }) => {
                // Only the newest preview matters. This makes key-repeat constant-space
                // even when a large diff is slower than navigation.
                self.preview = Some(command);
            }
            command @ WorkerCommand::LoadHistory { .. } => self.history = Some(command),
            command @ (WorkerCommand::LoadGitHubRepositories { .. }
            | WorkerCommand::LookupPullRequest { .. }) => {
                self.pull_request = Some(command);
            }
            WorkerCommand::Shutdown => self.shutdown = true,
        }
    }

    fn pop(&mut self) -> Option<WorkerCommand> {
        // Explicit user work and visible previews win over background pagination.
        self.operations
            .pop_front()
            .or_else(|| self.branches.take())
            .or_else(|| self.preview.take())
            .or_else(|| self.pull_request.take())
            .or_else(|| self.refresh.take())
            .or_else(|| self.history.take())
    }
}

struct SharedMailbox {
    state: Mutex<Mailbox>,
    ready: Condvar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerLane {
    Background,
    GitHubMetadata,
    LocalPreview,
    PullRequestPreview,
}

fn worker_lane(command: &WorkerCommand) -> WorkerLane {
    match command {
        WorkerCommand::LoadDiff { .. }
        | WorkerCommand::LoadCommit { .. }
        | WorkerCommand::LoadBranchDiff { .. }
        | WorkerCommand::LoadStashDiff { .. } => WorkerLane::LocalPreview,
        WorkerCommand::LoadGitHubRepositories { .. } | WorkerCommand::LookupPullRequest { .. } => {
            WorkerLane::GitHubMetadata
        }
        WorkerCommand::LoadPullRequest { .. } => WorkerLane::PullRequestPreview,
        _ => WorkerLane::Background,
    }
}

pub struct GitWorker {
    mailbox: Arc<SharedMailbox>,
    github_mailbox: Arc<SharedMailbox>,
    local_preview_mailbox: Arc<SharedMailbox>,
    pull_request_preview_mailbox: Arc<SharedMailbox>,
    events: Receiver<WorkerEvent>,
}

impl GitWorker {
    pub fn start(repository: Repository) -> Self {
        let mailbox = new_mailbox();
        let github_mailbox = new_mailbox();
        let local_preview_mailbox = new_mailbox();
        let pull_request_preview_mailbox = new_mailbox();
        let worker_mailbox = Arc::clone(&mailbox);
        let worker_github_mailbox = Arc::clone(&github_mailbox);
        let worker_local_preview_mailbox = Arc::clone(&local_preview_mailbox);
        let worker_pull_request_preview_mailbox = Arc::clone(&pull_request_preview_mailbox);
        let github_repository = repository.clone_for_worker();
        let local_preview_repository = repository.clone_for_worker();
        let pull_request_preview_repository = repository.clone_for_worker();
        let (event_tx, event_rx) = unbounded();
        let github_events = event_tx.clone();
        let local_preview_events = event_tx.clone();
        let pull_request_preview_events = event_tx.clone();
        thread::Builder::new()
            .name("quinjet-git".to_owned())
            .spawn(move || run_worker(repository, worker_mailbox, event_tx))
            .expect("failed to start Git worker");
        thread::Builder::new()
            .name("quinjet-github".to_owned())
            .spawn(move || run_worker(github_repository, worker_github_mailbox, github_events))
            .expect("failed to start GitHub metadata worker");
        thread::Builder::new()
            .name("quinjet-preview".to_owned())
            .spawn(move || {
                run_worker(
                    local_preview_repository,
                    worker_local_preview_mailbox,
                    local_preview_events,
                )
            })
            .expect("failed to start local preview worker");
        thread::Builder::new()
            .name("quinjet-pr-preview".to_owned())
            .spawn(move || {
                run_worker(
                    pull_request_preview_repository,
                    worker_pull_request_preview_mailbox,
                    pull_request_preview_events,
                )
            })
            .expect("failed to start pull-request preview worker");
        Self {
            mailbox,
            github_mailbox,
            local_preview_mailbox,
            pull_request_preview_mailbox,
            events: event_rx,
        }
    }

    /// Queue work without blocking the render thread. Read requests occupy fixed
    /// mailbox slots and replace obsolete requests; repository mutations remain an
    /// ordered queue and are additionally serialized by the app's busy state.
    pub fn send(&self, command: WorkerCommand) -> bool {
        let target = match worker_lane(&command) {
            WorkerLane::GitHubMetadata => &self.github_mailbox,
            WorkerLane::LocalPreview => &self.local_preview_mailbox,
            WorkerLane::PullRequestPreview => &self.pull_request_preview_mailbox,
            WorkerLane::Background => &self.mailbox,
        };
        let Ok(mut mailbox) = target.state.lock() else {
            return false;
        };
        if mailbox.shutdown {
            return false;
        }
        mailbox.push(command);
        drop(mailbox);
        target.ready.notify_one();
        true
    }

    pub fn events(&self) -> &Receiver<WorkerEvent> {
        &self.events
    }
}

impl Drop for GitWorker {
    fn drop(&mut self) {
        shutdown_mailbox(&self.mailbox);
        shutdown_mailbox(&self.github_mailbox);
        shutdown_mailbox(&self.local_preview_mailbox);
        shutdown_mailbox(&self.pull_request_preview_mailbox);
    }
}

fn new_mailbox() -> Arc<SharedMailbox> {
    Arc::new(SharedMailbox {
        state: Mutex::new(Mailbox::default()),
        ready: Condvar::new(),
    })
}

fn shutdown_mailbox(mailbox: &SharedMailbox) {
    let Ok(mut state) = mailbox.state.lock() else {
        return;
    };
    state.push(WorkerCommand::Shutdown);
    drop(state);
    mailbox.ready.notify_one();
}

fn run_worker(repository: Repository, mailbox: Arc<SharedMailbox>, events: Sender<WorkerEvent>) {
    while let Some(command) = next_command(&mailbox) {
        let event = match command {
            WorkerCommand::Refresh { generation } => WorkerEvent::Status {
                generation,
                result: repository.status().map_err(format_error),
            },
            WorkerCommand::LoadDiff {
                generation,
                changes,
                expanded,
            } => WorkerEvent::Diff {
                generation,
                result: repository
                    .diff_for_changes(&changes, expanded)
                    .map_err(format_error),
            },
            WorkerCommand::LoadHistory {
                generation,
                revision,
                skip,
                limit,
            } => WorkerEvent::History {
                generation,
                skip,
                result: repository
                    .history(&revision, skip, limit)
                    .map_err(format_error),
            },
            WorkerCommand::LoadCommit { generation, commit } => WorkerEvent::CommitDetail {
                generation,
                result: repository.commit_detail(&commit).map_err(format_error),
            },
            WorkerCommand::LoadBranchDiff {
                generation,
                branch,
                expanded,
            } => WorkerEvent::BranchDiff {
                generation,
                result: repository
                    .branch_diff(&branch, expanded)
                    .map_err(format_error),
            },
            WorkerCommand::LoadStashDiff { generation, stash } => WorkerEvent::StashDiff {
                generation,
                result: repository.stash_diff(&stash).map_err(format_error),
            },
            WorkerCommand::LoadGitHubRepositories {
                generation,
                refresh,
            } => WorkerEvent::GitHubRepositories {
                generation,
                result: repository
                    .github_repositories(refresh)
                    .map_err(format_error),
            },
            WorkerCommand::LookupPullRequest {
                generation,
                repositories,
                repository: selected_repository,
                number,
                refresh,
            } => WorkerEvent::PullRequestLookup {
                generation,
                result: {
                    let _ = events.send(WorkerEvent::PullRequestProgress {
                        generation,
                        diff: false,
                        progress: PullRequestProgress::LoadingMetadata,
                    });
                    repository
                        .pull_request_lookup(
                            &repositories,
                            selected_repository.as_deref(),
                            number,
                            refresh,
                        )
                        .map_err(format_error)
                },
            },
            WorkerCommand::LoadPullRequest {
                generation,
                pull_request,
                file_page,
                file_page_size,
            } => WorkerEvent::PullRequestDiff {
                generation,
                result: repository
                    .pull_request_diff_with_progress(
                        &pull_request,
                        file_page,
                        file_page_size,
                        |progress| {
                            let _ = events.send(WorkerEvent::PullRequestProgress {
                                generation,
                                diff: true,
                                progress,
                            });
                        },
                    )
                    .map_err(format_error),
            },
            WorkerCommand::LoadBranches { generation } => WorkerEvent::Branches {
                generation,
                result: repository.branches().map_err(format_error),
            },
            WorkerCommand::LoadHistoryBranches { generation } => WorkerEvent::HistoryBranches {
                generation,
                result: repository.history_branches().map_err(format_error),
            },
            WorkerCommand::LoadStashes { generation } => WorkerEvent::Stashes {
                generation,
                result: repository.stashes().map_err(format_error),
            },
            WorkerCommand::Operate { id, operation } => {
                let label = operation.label().to_owned();
                let changes_history = operation.changes_history();
                WorkerEvent::OperationFinished {
                    id,
                    label,
                    changes_history,
                    result: repository.perform(&operation).map_err(format_error),
                }
            }
            WorkerCommand::Shutdown => break,
        };

        if events.send(event).is_err() {
            break;
        }
    }
}

fn next_command(mailbox: &SharedMailbox) -> Option<WorkerCommand> {
    let mut state = mailbox.state.lock().ok()?;
    loop {
        if state.shutdown {
            return None;
        }
        if let Some(command) = state.pop() {
            return Some(command);
        }
        state = mailbox.ready.wait(state).ok()?;
    }
}

fn format_error(error: anyhow::Error) -> String {
    format!("{error:#}")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::git::status::{ChangeArea, ChangeStatus};

    #[test]
    fn mailbox_coalesces_previews_and_refreshes() {
        let mut mailbox = Mailbox::default();
        mailbox.push(WorkerCommand::Refresh { generation: 1 });
        mailbox.push(WorkerCommand::Refresh { generation: 2 });
        mailbox.push(WorkerCommand::LoadDiff {
            generation: 1,
            changes: vec![Change {
                path: PathBuf::from("old.rs"),
                original_path: None,
                area: ChangeArea::Unstaged,
                status: ChangeStatus::Modified,
            }],
            expanded: false,
        });
        mailbox.push(WorkerCommand::LoadDiff {
            generation: 2,
            changes: vec![Change {
                path: PathBuf::from("new.rs"),
                original_path: None,
                area: ChangeArea::Unstaged,
                status: ChangeStatus::Modified,
            }],
            expanded: true,
        });

        assert!(matches!(
            mailbox.pop(),
            Some(WorkerCommand::LoadDiff { generation: 2, .. })
        ));
        assert!(matches!(
            mailbox.pop(),
            Some(WorkerCommand::Refresh { generation: 2 })
        ));
        assert!(mailbox.pop().is_none());
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
        let local_preview = WorkerCommand::LoadDiff {
            generation: 2,
            changes: Vec::new(),
            expanded: false,
        };
        assert_eq!(worker_lane(&local_preview), WorkerLane::LocalPreview);

        let request = PullRequest {
            number: 1,
            title: String::new(),
            description: String::new(),
            author: String::new(),
            state: "OPEN".to_owned(),
            is_draft: false,
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
        let pr_preview = WorkerCommand::LoadPullRequest {
            generation: 3,
            pull_request: Box::new(request),
            file_page: 1,
            file_page_size: 20,
        };
        assert_eq!(worker_lane(&pr_preview), WorkerLane::PullRequestPreview);
        assert_eq!(
            worker_lane(&WorkerCommand::Refresh { generation: 4 }),
            WorkerLane::Background
        );
        assert_eq!(
            worker_lane(&WorkerCommand::LookupPullRequest {
                generation: 5,
                repositories: Vec::new(),
                repository: None,
                number: 42,
                refresh: false,
            }),
            WorkerLane::GitHubMetadata
        );
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
}
