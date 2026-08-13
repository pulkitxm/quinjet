use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use super::diff::DiffDocument;
use super::history::Commit;
use super::status::{Change, RepoStatus};
use super::{Branch, GitOperation, Repository};

#[derive(Debug)]
pub enum WorkerCommand {
    Refresh {
        generation: u64,
    },
    LoadDiff {
        generation: u64,
        change: Change,
    },
    LoadHistory {
        generation: u64,
        skip: usize,
        limit: usize,
    },
    LoadCommit {
        generation: u64,
        commit: Commit,
    },
    LoadBranches {
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
    Branches {
        generation: u64,
        result: Result<Vec<Branch>, String>,
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
    shutdown: bool,
}

impl Mailbox {
    fn push(&mut self, command: WorkerCommand) {
        match command {
            command @ WorkerCommand::Operate { .. } => self.operations.push_back(command),
            command @ WorkerCommand::LoadBranches { .. } => self.branches = Some(command),
            command @ WorkerCommand::Refresh { .. } => self.refresh = Some(command),
            command @ (WorkerCommand::LoadDiff { .. } | WorkerCommand::LoadCommit { .. }) => {
                // Only the newest preview matters. This makes key-repeat constant-space
                // even when a large diff is slower than navigation.
                self.preview = Some(command);
            }
            command @ WorkerCommand::LoadHistory { .. } => self.history = Some(command),
            WorkerCommand::Shutdown => self.shutdown = true,
        }
    }

    fn pop(&mut self) -> Option<WorkerCommand> {
        // Explicit user work wins over background refresh and history queries.
        self.operations
            .pop_front()
            .or_else(|| self.branches.take())
            .or_else(|| self.refresh.take())
            .or_else(|| self.preview.take())
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
            WorkerCommand::LoadDiff { generation, change } => WorkerEvent::Diff {
                generation,
                result: repository.diff_for_change(&change).map_err(format_error),
            },
            WorkerCommand::LoadHistory {
                generation,
                skip,
                limit,
            } => WorkerEvent::History {
                generation,
                skip,
                result: repository.history(skip, limit).map_err(format_error),
            },
            WorkerCommand::LoadCommit { generation, commit } => WorkerEvent::CommitDetail {
                generation,
                result: repository.commit_detail(&commit).map_err(format_error),
            },
            WorkerCommand::LoadBranches { generation } => WorkerEvent::Branches {
                generation,
                result: repository.branches().map_err(format_error),
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
            change: Change {
                path: PathBuf::from("old.rs"),
                original_path: None,
                area: ChangeArea::Unstaged,
                status: ChangeStatus::Modified,
            },
        });
        mailbox.push(WorkerCommand::LoadDiff {
            generation: 2,
            change: Change {
                path: PathBuf::from("new.rs"),
                original_path: None,
                area: ChangeArea::Unstaged,
                status: ChangeStatus::Modified,
            },
        });

        assert!(matches!(
            mailbox.pop(),
            Some(WorkerCommand::Refresh { generation: 2 })
        ));
        assert!(matches!(
            mailbox.pop(),
            Some(WorkerCommand::LoadDiff { generation: 2, .. })
        ));
        assert!(mailbox.pop().is_none());
    }

    #[test]
    fn mailbox_prioritizes_user_operations() {
        let mut mailbox = Mailbox::default();
        mailbox.push(WorkerCommand::LoadHistory {
            generation: 1,
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
