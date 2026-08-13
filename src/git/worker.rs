use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use super::diff::DiffDocument;
use super::github::{PullRequest, PullRequestBatch, PullRequestSnapshot};
use super::history::Commit;
use super::status::{Change, RepoStatus};
use super::{Branch, GitOperation, HistoryBranch, Repository};

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
    LoadPullRequests {
        generation: u64,
        repositories: Vec<super::github::GitHubRepository>,
        repository: Option<Box<super::github::GitHubRepository>>,
        cursor: Option<String>,
        refresh: bool,
    },
    LookupPullRequest {
        generation: u64,
        repositories: Vec<super::github::GitHubRepository>,
        repository: Box<super::github::GitHubRepository>,
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
    PullRequestBatch {
        generation: u64,
        result: Result<PullRequestBatch, String>,
    },
    PullRequestLookup {
        generation: u64,
        result: Result<PullRequestSnapshot, String>,
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
    pull_requests: Option<WorkerCommand>,
    shutdown: bool,
}

impl Mailbox {
    fn push(&mut self, command: WorkerCommand) {
        match command {
            command @ WorkerCommand::Operate { .. } => self.operations.push_back(command),
            command @ (WorkerCommand::LoadBranches { .. }
            | WorkerCommand::LoadHistoryBranches { .. }) => self.branches = Some(command),
            command @ WorkerCommand::Refresh { .. } => self.refresh = Some(command),
            command @ (WorkerCommand::LoadDiff { .. }
            | WorkerCommand::LoadCommit { .. }
            | WorkerCommand::LoadPullRequest { .. }) => {
                // Only the newest preview matters. This makes key-repeat constant-space
                // even when a large diff is slower than navigation.
                self.preview = Some(command);
            }
            command @ WorkerCommand::LoadHistory { .. } => self.history = Some(command),
            command @ (WorkerCommand::LoadPullRequests { .. }
            | WorkerCommand::LookupPullRequest { .. }) => {
                self.pull_requests = Some(command);
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
            .or_else(|| self.pull_requests.take())
            .or_else(|| self.refresh.take())
            .or_else(|| self.history.take())
    }
}

struct SharedMailbox {
    state: Mutex<Mailbox>,
    ready: Condvar,
}

pub struct GitWorker {
    mailbox: Arc<SharedMailbox>,
    events: Receiver<WorkerEvent>,
}

impl GitWorker {
    pub fn start(repository: Repository) -> Self {
        let mailbox = Arc::new(SharedMailbox {
            state: Mutex::new(Mailbox::default()),
            ready: Condvar::new(),
        });
        let worker_mailbox = Arc::clone(&mailbox);
        let (event_tx, event_rx) = unbounded();
        thread::Builder::new()
            .name("quinjet-git".to_owned())
            .spawn(move || run_worker(repository, worker_mailbox, event_tx))
            .expect("failed to start Git worker");
        Self {
            mailbox,
            events: event_rx,
        }
    }

    /// Queue work without blocking the render thread. Read requests occupy fixed
    /// mailbox slots and replace obsolete requests; repository mutations remain an
    /// ordered queue and are additionally serialized by the app's busy state.
    pub fn send(&self, command: WorkerCommand) -> bool {
        let Ok(mut mailbox) = self.mailbox.state.lock() else {
            return false;
        };
        if mailbox.shutdown {
            return false;
        }
        mailbox.push(command);
        drop(mailbox);
        self.mailbox.ready.notify_one();
        true
    }

    pub fn events(&self) -> &Receiver<WorkerEvent> {
        &self.events
    }
}

impl Drop for GitWorker {
    fn drop(&mut self) {
        let Ok(mut mailbox) = self.mailbox.state.lock() else {
            return;
        };
        mailbox.push(WorkerCommand::Shutdown);
        drop(mailbox);
        self.mailbox.ready.notify_one();
    }
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
            WorkerCommand::LoadPullRequests {
                generation,
                repositories,
                repository: selected_repository,
                cursor,
                refresh,
            } => WorkerEvent::PullRequestBatch {
                generation,
                result: repository
                    .pull_request_batch(
                        &repositories,
                        selected_repository.as_deref(),
                        cursor.as_deref(),
                        refresh,
                    )
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
                result: repository
                    .pull_request_lookup(&repositories, &selected_repository, number, refresh)
                    .map_err(format_error),
            },
            WorkerCommand::LoadPullRequest {
                generation,
                pull_request,
                file_page,
                file_page_size,
            } => WorkerEvent::PullRequestDiff {
                generation,
                result: repository
                    .pull_request_diff(&pull_request, file_page, file_page_size)
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
    fn mailbox_coalesces_pull_request_lists_and_prioritizes_them_over_history() {
        let mut mailbox = Mailbox::default();
        mailbox.push(WorkerCommand::LoadPullRequests {
            generation: 1,
            repositories: Vec::new(),
            repository: None,
            cursor: None,
            refresh: false,
        });
        mailbox.push(WorkerCommand::LoadPullRequests {
            generation: 2,
            repositories: Vec::new(),
            repository: None,
            cursor: Some("next-cursor".to_owned()),
            refresh: false,
        });
        mailbox.push(WorkerCommand::LoadHistory {
            generation: 1,
            revision: "HEAD".to_owned(),
            skip: 0,
            limit: 300,
        });

        assert!(matches!(
            mailbox.pop(),
            Some(WorkerCommand::LoadPullRequests {
                generation: 2,
                cursor: Some(cursor),
                ..
            }) if cursor == "next-cursor"
        ));
        assert!(matches!(
            mailbox.pop(),
            Some(WorkerCommand::LoadHistory { generation: 1, .. })
        ));
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
