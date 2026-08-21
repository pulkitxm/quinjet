use super::*;

pub(super) fn new_mailbox() -> Arc<SharedMailbox> {
    Arc::new(SharedMailbox {
        state: Mutex::new(Mailbox::default()),
        ready: Condvar::new(),
    })
}

pub(super) fn shutdown_mailbox(mailbox: &SharedMailbox) {
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
pub(super) fn run_warm_worker(
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
pub(super) fn run_worker(
    repository: &Repository,
    mailbox: &Arc<SharedMailbox>,
    events: &Sender<WorkerEvent>,
) {
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
            WorkerCommand::LoadLocalGitHubRepository => WorkerEvent::LocalGitHubRepository {
                result: answer(
                    session
                        .execute(Command::LocalGitHubRepository)
                        .and_then(Outcome::local_github_repository),
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
            WorkerCommand::LoadWorktrees { generation } => WorkerEvent::Worktrees {
                generation,
                result: answer(
                    session
                        .execute(Command::Worktrees)
                        .and_then(Outcome::worktrees),
                ),
            },
            WorkerCommand::LoadRecentProjects { generation } => WorkerEvent::RecentProjects {
                generation,
                result: answer(
                    session
                        .execute(Command::RecentProjects)
                        .and_then(Outcome::recent_projects),
                ),
            },
            WorkerCommand::Operate { id, operation } => {
                let label = operation.label().to_owned();
                let changes_history = operation.changes_history();
                WorkerEvent::OperationFinished {
                    id,
                    label,
                    changes_history,
                    refresh_pull_request: false,
                    result: answer(
                        session
                            .execute(Command::Operate(operation))
                            .and_then(Outcome::operation)
                            .map(|(_, _, message)| message),
                    ),
                }
            }
            WorkerCommand::OperatePullRequest {
                id,
                pull_request,
                operation,
            } => {
                let label = operation.label().to_owned();
                WorkerEvent::OperationFinished {
                    id,
                    label,
                    changes_history: false,
                    refresh_pull_request: true,
                    result: answer(
                        session
                            .execute(Command::OperatePullRequest {
                                pull_request,
                                operation,
                            })
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

pub(super) fn next_command(mailbox: &SharedMailbox) -> Option<WorkerCommand> {
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

pub(super) fn answer<T>(result: anyhow::Result<T>) -> Result<T, String> {
    result.map_err(|error| format!("{error:#}"))
}
