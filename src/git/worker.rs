use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use super::diff::DiffDocument;
use super::github::{
    CheckRunLog, PullRequest, PullRequestCheck, PullRequestChecks, PullRequestConversation,
    PullRequestDiffIndex, PullRequestProgress, PullRequestSnapshot,
};
use super::history::Commit;
use super::status::RepoStatus;
use super::{Branch, GitOperation, HistoryBranch, LocalDiffRequest, Repository, Stash};
use crate::cli::{Command, Outcome, Session};

#[derive(Debug)]
pub(crate) enum WorkerCommand {
    Refresh {
        generation: u64,
    },
    PrepareLocalDiff {
        generation: u64,
        request: Box<LocalDiffRequest>,
    },
    LoadLocalDiffFile {
        generation: u64,
        workspace_generation: u64,
        path: PathBuf,
    },
    LoadHistory {
        generation: u64,
        revision: String,
        skip: usize,
        limit: usize,
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
    PreparePullRequest {
        generation: u64,
        pull_request: Box<PullRequest>,
    },
    LoadPullRequestFile {
        generation: u64,
        workspace_generation: u64,
        path: PathBuf,
    },
    /// Background fill for the rest of a prepared pull request. It carries no
    /// preview generation because it never replaces what the reader is looking
    /// at; the workspace it was prepared against is the only thing that can
    /// make its results stale.
    LoadPullRequestFileBatch {
        workspace_generation: u64,
        paths: Vec<PathBuf>,
    },
    LoadPullRequestChecks {
        generation: u64,
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    LoadPullRequestConversation {
        generation: u64,
        pull_request: Box<PullRequest>,
    },
    LoadCheckRunLog {
        generation: u64,
        pull_request: Box<PullRequest>,
        check: Box<PullRequestCheck>,
    },
    /// Warm every finished run's log so that opening any of them is instant.
    /// It carries no generation because it changes nothing on screen.
    PrefetchCheckRunLogs {
        generation: u64,
        pull_request: Box<PullRequest>,
        checks: Vec<PullRequestCheck>,
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
pub(crate) enum WorkerEvent {
    Status {
        generation: u64,
        result: Result<RepoStatus, String>,
    },
    LocalDiffIndex {
        generation: u64,
        result: Result<super::diff::DiffIndex, String>,
    },
    LocalDiffFile {
        generation: u64,
        workspace_generation: u64,
        path: PathBuf,
        result: Result<DiffDocument, String>,
    },
    History {
        generation: u64,
        skip: usize,
        result: Result<Vec<Commit>, String>,
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
    PullRequestIndex {
        generation: u64,
        result: Result<PullRequestDiffIndex, String>,
    },
    PullRequestDiff {
        generation: u64,
        result: Result<DiffDocument, String>,
    },
    PullRequestDiffBatch {
        workspace_generation: u64,
        result: Result<Vec<(PathBuf, DiffDocument)>, String>,
    },
    PullRequestChecks {
        generation: u64,
        result: Result<PullRequestChecks, String>,
    },
    PullRequestConversation {
        generation: u64,
        result: Result<PullRequestConversation, String>,
    },
    CheckRunLog {
        generation: u64,
        result: Result<CheckRunLog, String>,
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
    repositories: Option<WorkerCommand>,
    prefetch: Option<WorkerCommand>,
    checks: Option<WorkerCommand>,
    conversation: Option<WorkerCommand>,
    check_log: Option<WorkerCommand>,
    warm: Option<WorkerCommand>,
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
            command @ (WorkerCommand::PrepareLocalDiff { .. }
            | WorkerCommand::LoadLocalDiffFile { .. }
            | WorkerCommand::PreparePullRequest { .. }
            | WorkerCommand::LoadPullRequestFile { .. }) => {
                self.preview = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestFileBatch { .. } => {
                self.prefetch = Some(command);
            }
            command @ WorkerCommand::LoadHistory { .. } => self.history = Some(command),
            command @ WorkerCommand::LoadGitHubRepositories { .. } => {
                self.repositories = Some(command);
            }
            command @ WorkerCommand::LookupPullRequest { .. } => {
                self.pull_request = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestChecks { .. } => self.checks = Some(command),
            command @ WorkerCommand::LoadPullRequestConversation { .. } => {
                self.conversation = Some(command);
            }
            command @ WorkerCommand::LoadCheckRunLog { .. } => self.check_log = Some(command),
            command @ WorkerCommand::PrefetchCheckRunLogs { .. } => {
                self.warm = Some(command);
            }
            WorkerCommand::Shutdown => self.shutdown = true,
        }
    }

    fn pop(&mut self) -> Option<WorkerCommand> {
        self.operations
            .pop_front()
            .or_else(|| self.branches.take())
            .or_else(|| self.preview.take())
            .or_else(|| self.repositories.take())
            .or_else(|| self.pull_request.take())
            .or_else(|| self.refresh.take())
            .or_else(|| self.check_log.take())
            .or_else(|| self.checks.take())
            .or_else(|| self.conversation.take())
            .or_else(|| self.history.take())
            .or_else(|| self.prefetch.take())
            .or_else(|| self.warm.take())
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
    Warm,
}

const fn worker_lane(command: &WorkerCommand) -> WorkerLane {
    match command {
        WorkerCommand::PrepareLocalDiff { .. } | WorkerCommand::LoadLocalDiffFile { .. } => {
            WorkerLane::LocalPreview
        }
        WorkerCommand::LoadGitHubRepositories { .. }
        | WorkerCommand::LookupPullRequest { .. }
        | WorkerCommand::LoadPullRequestChecks { .. }
        | WorkerCommand::LoadPullRequestConversation { .. }
        | WorkerCommand::LoadCheckRunLog { .. } => WorkerLane::GitHubMetadata,
        WorkerCommand::PrefetchCheckRunLogs { .. } => WorkerLane::Warm,
        WorkerCommand::PreparePullRequest { .. }
        | WorkerCommand::LoadPullRequestFile { .. }
        | WorkerCommand::LoadPullRequestFileBatch { .. } => WorkerLane::PullRequestPreview,
        _ => WorkerLane::Background,
    }
}

pub(crate) struct GitWorker {
    mailbox: Arc<SharedMailbox>,
    github_mailbox: Arc<SharedMailbox>,
    local_preview_mailbox: Arc<SharedMailbox>,
    pull_request_preview_mailbox: Arc<SharedMailbox>,
    warm_mailbox: Arc<SharedMailbox>,
    warm_generation: Arc<AtomicU64>,
    events: Receiver<WorkerEvent>,
}

impl GitWorker {
    #[expect(
        clippy::expect_used,
        reason = "the interface cannot run without its worker threads"
    )]
    pub(crate) fn start(repository: Repository) -> Self {
        let mailbox = new_mailbox();
        let github_mailbox = new_mailbox();
        let local_preview_mailbox = new_mailbox();
        let pull_request_preview_mailbox = new_mailbox();
        let warm_mailbox = new_mailbox();
        let warm_generation = Arc::new(AtomicU64::new(0));
        let worker_warm_generation = Arc::clone(&warm_generation);
        let worker_mailbox = Arc::clone(&mailbox);
        let worker_github_mailbox = Arc::clone(&github_mailbox);
        let worker_local_preview_mailbox = Arc::clone(&local_preview_mailbox);
        let worker_pull_request_preview_mailbox = Arc::clone(&pull_request_preview_mailbox);
        let worker_warm_mailbox = Arc::clone(&warm_mailbox);
        let github_repository = repository.clone_for_worker();
        let local_preview_repository = repository.clone_for_worker();
        let pull_request_preview_repository = repository.clone_for_worker();
        let warm_repository = repository.clone_for_worker();
        let (event_tx, event_rx) = unbounded();
        let github_events = event_tx.clone();
        let local_preview_events = event_tx.clone();
        let pull_request_preview_events = event_tx.clone();
        let warm_events = event_tx.clone();
        drop(
            thread::Builder::new()
                .name("quinjet-git".to_owned())
                .spawn(move || run_worker(&repository, &worker_mailbox, &event_tx))
                .expect("failed to start Git worker"),
        );
        drop(
            thread::Builder::new()
                .name("quinjet-github".to_owned())
                .spawn(move || {
                    run_worker(&github_repository, &worker_github_mailbox, &github_events);
                })
                .expect("failed to start GitHub metadata worker"),
        );
        drop(
            thread::Builder::new()
                .name("quinjet-preview".to_owned())
                .spawn(move || {
                    run_worker(
                        &local_preview_repository,
                        &worker_local_preview_mailbox,
                        &local_preview_events,
                    );
                })
                .expect("failed to start local preview worker"),
        );
        drop(
            thread::Builder::new()
                .name("quinjet-pr-preview".to_owned())
                .spawn(move || {
                    run_worker(
                        &pull_request_preview_repository,
                        &worker_pull_request_preview_mailbox,
                        &pull_request_preview_events,
                    );
                })
                .expect("failed to start pull-request preview worker"),
        );
        drop(
            thread::Builder::new()
                .name("quinjet-warm".to_owned())
                .spawn(move || {
                    run_warm_worker(
                        &warm_repository,
                        &worker_warm_mailbox,
                        &warm_events,
                        &worker_warm_generation,
                    );
                })
                .expect("failed to start log warm-up worker"),
        );
        Self {
            mailbox,
            github_mailbox,
            local_preview_mailbox,
            pull_request_preview_mailbox,
            warm_mailbox,
            warm_generation,
            events: event_rx,
        }
    }

    /// Queue work without blocking the render thread. Read requests occupy fixed
    /// mailbox slots and replace obsolete requests; repository mutations remain an
    /// ordered queue and are additionally serialized by the app's busy state.
    pub(crate) fn send(&self, mut command: WorkerCommand) -> bool {
        if let WorkerCommand::PrefetchCheckRunLogs { generation, .. } = &mut command {
            *generation = self.warm_generation.fetch_add(1, Ordering::SeqCst) + 1;
        }
        let target = match worker_lane(&command) {
            WorkerLane::GitHubMetadata => &self.github_mailbox,
            WorkerLane::LocalPreview => &self.local_preview_mailbox,
            WorkerLane::PullRequestPreview => &self.pull_request_preview_mailbox,
            WorkerLane::Warm => &self.warm_mailbox,
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

    pub(crate) const fn events(&self) -> &Receiver<WorkerEvent> {
        &self.events
    }
}

impl Drop for GitWorker {
    fn drop(&mut self) {
        shutdown_mailbox(&self.mailbox);
        shutdown_mailbox(&self.github_mailbox);
        shutdown_mailbox(&self.local_preview_mailbox);
        shutdown_mailbox(&self.pull_request_preview_mailbox);
        shutdown_mailbox(&self.warm_mailbox);
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

/// The warm-up lane runs one job at a time and answers to nothing but its own
/// generation, so a pull request the reader has left stops costing requests as
/// soon as another one asks to be warmed.
fn run_warm_worker(
    repository: &Repository,
    mailbox: &Arc<SharedMailbox>,
    _events: &Sender<WorkerEvent>,
    generation: &Arc<AtomicU64>,
) {
    let mut session = Session::new(repository.clone_for_worker());
    while let Some(command) = next_command(mailbox) {
        match command {
            WorkerCommand::PrefetchCheckRunLogs {
                generation: mine,
                pull_request,
                checks,
            } => {
                drop(session.execute_with(
                    Command::WarmCheckRunLogs {
                        pull_request,
                        checks,
                    },
                    &mut |_| {},
                    &|| generation.load(Ordering::SeqCst) == mine,
                ));
            }
            WorkerCommand::Shutdown => break,
            _ => {}
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
fn run_worker(repository: &Repository, mailbox: &Arc<SharedMailbox>, events: &Sender<WorkerEvent>) {
    let mut session = Session::new(repository.clone_for_worker());
    while let Some(command) = next_command(mailbox) {
        let event = match command {
            WorkerCommand::Refresh { generation } => WorkerEvent::Status {
                generation,
                result: answer(session.execute(Command::Status).and_then(Outcome::status)),
            },
            WorkerCommand::PrepareLocalDiff {
                generation,
                request,
            } => WorkerEvent::LocalDiffIndex {
                generation,
                result: answer(
                    session
                        .execute(Command::PrepareLocalDiff {
                            workspace: generation,
                            request,
                        })
                        .and_then(Outcome::local_diff_index),
                ),
            },
            WorkerCommand::LoadLocalDiffFile {
                generation,
                workspace_generation,
                path,
            } => WorkerEvent::LocalDiffFile {
                generation,
                workspace_generation,
                path: path.clone(),
                result: answer(
                    session
                        .execute(Command::LocalDiffFile {
                            workspace: workspace_generation,
                            path,
                        })
                        .and_then(Outcome::local_diff_file)
                        .map(|(_, document)| document),
                ),
            },
            WorkerCommand::LoadHistory {
                generation,
                revision,
                skip,
                limit,
            } => WorkerEvent::History {
                generation,
                skip,
                result: answer(
                    session
                        .execute(Command::History {
                            revision,
                            skip,
                            limit,
                        })
                        .and_then(Outcome::history),
                ),
            },
            WorkerCommand::LoadGitHubRepositories {
                generation,
                refresh,
            } => WorkerEvent::GitHubRepositories {
                generation,
                result: answer(
                    session
                        .execute(Command::GitHubRepositories { refresh })
                        .and_then(Outcome::github_repositories),
                ),
            },
            WorkerCommand::LookupPullRequest {
                generation,
                repositories,
                repository: selected_repository,
                number,
                refresh,
            } => WorkerEvent::PullRequestLookup {
                generation,
                result: answer(
                    session
                        .execute_with(
                            Command::PullRequestLookup {
                                repositories,
                                repository: selected_repository,
                                number,
                                refresh,
                            },
                            &mut |progress| {
                                drop(events.send(WorkerEvent::PullRequestProgress {
                                    generation,
                                    diff: false,
                                    progress,
                                }));
                            },
                            &|| true,
                        )
                        .and_then(Outcome::pull_request),
                ),
            },
            WorkerCommand::PreparePullRequest {
                generation,
                pull_request,
            } => WorkerEvent::PullRequestIndex {
                generation,
                result: answer(
                    session
                        .execute_with(
                            Command::PreparePullRequest {
                                workspace: generation,
                                pull_request,
                            },
                            &mut |progress| {
                                drop(events.send(WorkerEvent::PullRequestProgress {
                                    generation,
                                    diff: true,
                                    progress,
                                }));
                            },
                            &|| true,
                        )
                        .and_then(Outcome::pull_request_index),
                ),
            },
            WorkerCommand::LoadPullRequestFile {
                generation,
                workspace_generation,
                path,
            } => WorkerEvent::PullRequestDiff {
                generation,
                result: answer(
                    session
                        .execute(Command::PullRequestFile {
                            workspace: workspace_generation,
                            path,
                        })
                        .and_then(Outcome::pull_request_diff),
                ),
            },
            WorkerCommand::LoadPullRequestFileBatch {
                workspace_generation,
                paths,
            } => WorkerEvent::PullRequestDiffBatch {
                workspace_generation,
                result: answer(
                    session
                        .execute(Command::PullRequestFileBatch {
                            workspace: workspace_generation,
                            paths,
                        })
                        .and_then(Outcome::pull_request_diff_batch),
                ),
            },
            WorkerCommand::LoadPullRequestChecks {
                generation,
                pull_request,
                refresh,
            } => WorkerEvent::PullRequestChecks {
                generation,
                result: answer(
                    session
                        .execute(Command::PullRequestChecks {
                            pull_request,
                            refresh,
                        })
                        .and_then(Outcome::checks),
                ),
            },
            WorkerCommand::LoadPullRequestConversation {
                generation,
                pull_request,
            } => WorkerEvent::PullRequestConversation {
                generation,
                result: answer(
                    session
                        .execute(Command::PullRequestConversation { pull_request })
                        .and_then(Outcome::conversation),
                ),
            },
            WorkerCommand::LoadCheckRunLog {
                generation,
                pull_request,
                check,
            } => WorkerEvent::CheckRunLog {
                generation,
                result: answer(
                    session
                        .execute(Command::CheckRunLog {
                            pull_request,
                            check,
                        })
                        .and_then(Outcome::check_log),
                ),
            },
            WorkerCommand::PrefetchCheckRunLogs { .. } => continue,
            WorkerCommand::LoadBranches { generation } => WorkerEvent::Branches {
                generation,
                result: answer(
                    session
                        .execute(Command::Branches)
                        .and_then(Outcome::branches),
                ),
            },
            WorkerCommand::LoadHistoryBranches { generation } => WorkerEvent::HistoryBranches {
                generation,
                result: answer(
                    session
                        .execute(Command::HistoryBranches)
                        .and_then(Outcome::history_branches),
                ),
            },
            WorkerCommand::LoadStashes { generation } => WorkerEvent::Stashes {
                generation,
                result: answer(session.execute(Command::Stashes).and_then(Outcome::stashes)),
            },
            WorkerCommand::Operate { id, operation } => {
                let label = operation.label().to_owned();
                let changes_history = operation.changes_history();
                WorkerEvent::OperationFinished {
                    id,
                    label,
                    changes_history,
                    result: answer(
                        session
                            .execute(Command::Operate(operation))
                            .and_then(Outcome::operation)
                            .map(|(_, _, message)| message),
                    ),
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

fn answer<T>(result: anyhow::Result<T>) -> Result<T, String> {
    result.map_err(|error| format!("{error:#}"))
}

#[cfg(test)]
mod tests {
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
        assert_eq!(
            worker_lane(&WorkerCommand::LoadPullRequestChecks {
                generation: 6,
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
}
