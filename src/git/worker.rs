use std::thread;

use crossbeam_channel::{Receiver, Sender, TrySendError, bounded, unbounded};

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

pub struct GitWorker {
    commands: Sender<WorkerCommand>,
    events: Receiver<WorkerEvent>,
}

pub enum SendOutcome {
    Queued,
    Full(Box<WorkerCommand>),
    Disconnected,
}

impl GitWorker {
    pub fn start(repository: Repository) -> Self {
        // A small bounded queue applies backpressure if key-repeat produces requests
        // faster than Git can satisfy them. The UI uses try_send and remains non-blocking.
        let (command_tx, command_rx) = bounded(32);
        let (event_tx, event_rx) = unbounded();
        thread::Builder::new()
            .name("quinjet-git".to_owned())
            .spawn(move || run_worker(repository, command_rx, event_tx))
            .expect("failed to start Git worker");
        Self {
            commands: command_tx,
            events: event_rx,
        }
    }

    pub fn try_send(&self, command: WorkerCommand) -> SendOutcome {
        match self.commands.try_send(command) {
            Ok(()) => SendOutcome::Queued,
            Err(TrySendError::Full(command)) => SendOutcome::Full(Box::new(command)),
            Err(TrySendError::Disconnected(_)) => SendOutcome::Disconnected,
        }
    }

    pub fn events(&self) -> &Receiver<WorkerEvent> {
        &self.events
    }
}

impl Drop for GitWorker {
    fn drop(&mut self) {
        let _ = self.commands.try_send(WorkerCommand::Shutdown);
    }
}

fn run_worker(
    repository: Repository,
    commands: Receiver<WorkerCommand>,
    events: Sender<WorkerEvent>,
) {
    while let Ok(command) = commands.recv() {
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

fn format_error(error: anyhow::Error) -> String {
    format!("{error:#}")
}
