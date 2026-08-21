use super::*;

pub(crate) struct GitWorker {
    mailbox: Arc<SharedMailbox>,
    github_mailbox: Arc<SharedMailbox>,
    conversation_mailbox: Arc<SharedMailbox>,
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
        let conversation_mailbox = new_mailbox();
        let local_preview_mailbox = new_mailbox();
        let pull_request_preview_mailbox = new_mailbox();
        let warm_mailbox = new_mailbox();
        let warm_generation = Arc::new(AtomicU64::new(0));
        let worker_warm_generation = Arc::clone(&warm_generation);
        let worker_mailbox = Arc::clone(&mailbox);
        let worker_github_mailbox = Arc::clone(&github_mailbox);
        let worker_conversation_mailbox = Arc::clone(&conversation_mailbox);
        let worker_local_preview_mailbox = Arc::clone(&local_preview_mailbox);
        let worker_pull_request_preview_mailbox = Arc::clone(&pull_request_preview_mailbox);
        let worker_warm_mailbox = Arc::clone(&warm_mailbox);
        let github_repository = repository.clone_for_worker();
        let conversation_repository = repository.clone_for_worker();
        let local_preview_repository = repository.clone_for_worker();
        let pull_request_preview_repository = repository.clone_for_worker();
        let warm_repository = repository.clone_for_worker();
        let (event_tx, event_rx) = unbounded();
        let github_events = event_tx.clone();
        let conversation_events = event_tx.clone();
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
                .name("quinjet-conversation".to_owned())
                .spawn(move || {
                    run_worker(
                        &conversation_repository,
                        &worker_conversation_mailbox,
                        &conversation_events,
                    );
                })
                .expect("failed to start conversation worker"),
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
            conversation_mailbox,
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
            WorkerLane::Conversation => &self.conversation_mailbox,
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
        shutdown_mailbox(&self.conversation_mailbox);
        shutdown_mailbox(&self.local_preview_mailbox);
        shutdown_mailbox(&self.pull_request_preview_mailbox);
        shutdown_mailbox(&self.warm_mailbox);
    }
}
