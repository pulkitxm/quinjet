use super::*;

#[test]
fn pull_request_prefetch_batches_by_estimated_patch_size() {
    let counts = |additions: usize| {
        Some(DiffLineCounts {
            additions,
            deletions: 0,
            binary: false,
        })
    };
    let file = |path: &str, additions: usize| PullRequestFile {
        path: PathBuf::from(path),
        old_path: None,
        status: PullRequestFileStatus::Modified,
        counts: counts(additions),
    };
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_workspace_generation = Some(10);
    app.pull_request_files = vec![
        file("src/huge.rs", 200_000),
        file("src/small.rs", 10),
        file("src/tiny.rs", 5),
    ];

    let mut effects = Vec::new();
    app.request_pull_request_prefetch(&mut effects);
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch { workspace_generation: 10, paths }
                    if paths == &[PathBuf::from("src/huge.rs")]
            )
        ),
        "a file estimated past the byte budget travels alone"
    );

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestDiffBatch {
            workspace_generation: 10,
            result: Ok(vec![(
                PathBuf::from("src/huge.rs"),
                indexed_document(&["src/huge.rs"]),
            )]),
        },
        Instant::now(),
    );
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch { workspace_generation: 10, paths }
                    if paths == &[PathBuf::from("src/small.rs"), PathBuf::from("src/tiny.rs")]
            )
        ),
        "small files share the next batch"
    );
}

#[test]
fn prefetch_starts_at_the_files_viewport_and_wraps_around() {
    let counts = |additions: usize| {
        Some(DiffLineCounts {
            additions,
            deletions: 0,
            binary: false,
        })
    };
    let file = |path: &str| PullRequestFile {
        path: PathBuf::from(path),
        old_path: None,
        status: PullRequestFileStatus::Modified,
        counts: counts(5),
    };
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_workspace_generation = Some(10);
    app.view = View::PullRequests;
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_files = vec![file("a.rs"), file("b.rs"), file("c.rs"), file("d.rs")];
    let _ = app.pull_request_tree_entries();
    app.sidebar_offset = 2;

    let mut effects = Vec::new();
    app.request_pull_request_prefetch(&mut effects);
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch { workspace_generation: 10, paths }
                    if paths == &[
                        PathBuf::from("c.rs"),
                        PathBuf::from("d.rs"),
                        PathBuf::from("a.rs"),
                        PathBuf::from("b.rs"),
                    ]
            )
        ),
        "fill starts at the visible file and wraps around the index"
    );
}

#[test]
fn pull_request_prefetch_retries_once_after_a_failure() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_workspace_generation = Some(10);
    app.pull_request_files = vec![PullRequestFile {
        path: PathBuf::from("src/first.rs"),
        old_path: None,
        status: PullRequestFileStatus::Modified,
        counts: None,
    }];
    app.pull_request_prefetching = true;

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestDiffBatch {
            workspace_generation: 10,
            result: Err("transient failure".to_owned()),
        },
        Instant::now(),
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestFileBatch {
                workspace_generation: 10,
                paths,
            } if paths == &[PathBuf::from("src/first.rs")]
        )
    ));
    assert!(app.pull_request_prefetch_retrying);

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestDiffBatch {
            workspace_generation: 10,
            result: Err("persistent failure".to_owned()),
        },
        Instant::now(),
    );
    assert!(effects.is_empty());
    assert!(!app.pull_request_prefetching);
    assert!(!app.pull_request_prefetch_retrying);
}

#[test]
fn a_live_poll_refreshes_a_pull_request_without_disturbing_the_reader() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Checks", "acme/widget"));
    app.pull_request_exact_number = Some(8);
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
    app.pull_request_check_cursor = Some(0);
    app.content_scroll = 40;

    let mut effects = Vec::new();
    app.refresh_pull_request_live(now, false, &mut effects);

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(
            command.as_ref(),
            WorkerCommand::LookupPullRequest { number: 8, refresh: true, .. }
        )
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestChecks { .. })
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestConversation { .. })
    )));
    assert!(
        !effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command)
                if matches!(command.as_ref(), WorkerCommand::LoadCheckRunLog { .. })
        )),
        "a finished run's log never changes, so a poll does not re-read it"
    );
    assert!(
        app.pull_request.is_some(),
        "the loaded pull request stays on screen while the poll runs"
    );
    assert_eq!(app.pull_request_section, PullRequestSection::Files);
    assert_eq!(app.content_scroll, 40);
    assert!(app.pull_request_progress.is_none());
}

#[test]
fn conversation_refresh_replays_when_the_previous_read_finishes() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request = Some(pull_request(8, "Conversation", "acme/widget"));
    let mut first = Vec::new();
    app.request_pull_request_conversation(true, &mut first);
    let generation = app.pull_request_conversation_generation;

    let mut coalesced = Vec::new();
    app.request_pull_request_conversation(true, &mut coalesced);
    assert!(coalesced.is_empty());
    assert!(app.pull_request_conversation_refresh_again);

    let replayed = app.handle_worker_event(
        WorkerEvent::PullRequestConversation {
            generation,
            result: Ok(PullRequestConversation::default()),
        },
        Instant::now(),
    );
    assert!(matches!(
        replayed.as_slice(),
        [AppEffect::Git(command)]
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestConversation { .. })
    ));
    assert!(app.pull_request_conversation_loading);
}

#[test]
fn parsed_pull_request_documents_evict_oldest_entries_by_size() {
    let mut app = App::new("/tmp/repo", "repo");
    let mut document = indexed_document(&["src/file.rs"]);
    document.title = "x".repeat(1_024);
    let document_size = diff_document_size(&document);

    for name in ["first.rs", "second.rs", "third.rs"] {
        app.cache_pull_request_document(PathBuf::from(name), document.clone());
    }
    app.prune_pull_request_documents(document_size.saturating_mul(2));

    assert_eq!(app.pull_request_documents.len(), 2);
    assert!(
        !app.pull_request_documents
            .contains_key(Path::new("first.rs"))
    );
    assert!(
        app.pull_request_documents
            .contains_key(Path::new("second.rs"))
    );
    assert!(
        app.pull_request_documents
            .contains_key(Path::new("third.rs"))
    );
}

#[test]
fn a_fast_tick_only_speeds_up_the_reads_that_change_that_fast() {
    let mut app = App::new("/tmp/repo", "repo");
    let start = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Running", "acme/widget"));
    app.pull_request_exact_number = Some(8);
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Pending)];
    app.pull_request_check_cursor = Some(0);

    let mut first = Vec::new();
    app.refresh_pull_request_live(start, false, &mut first);
    let command_count = |effects: &[AppEffect], name: &str| {
        effects
            .iter()
            .filter(|effect| match effect {
                AppEffect::Git(command) => match name {
                    "checks" => {
                        matches!(
                            command.as_ref(),
                            WorkerCommand::LoadPullRequestChecks { .. }
                        )
                    }
                    "conversation" => matches!(
                        command.as_ref(),
                        WorkerCommand::LoadPullRequestConversation { .. }
                    ),
                    _ => matches!(command.as_ref(), WorkerCommand::LoadCheckRunLog { .. }),
                },
                AppEffect::Copy(_)
                | AppEffect::SetMouseCapture(_)
                | AppEffect::Open(_)
                | AppEffect::SwitchRepository(_)
                | AppEffect::OpenRepositoryTab(_)
                | AppEffect::SwitchSshMachine(_)
                | AppEffect::ActivateRepositoryTab(_)
                | AppEffect::ReorderRepositoryTab { .. }
                | AppEffect::CloseRepositoryTab(_)
                | AppEffect::CloseOtherRepositoryTabs(_)
                | AppEffect::CloseAllRepositoryTabs
                | AppEffect::Quit => false,
            })
            .count()
    };
    assert_eq!(command_count(&first, "checks"), 1);
    assert_eq!(command_count(&first, "conversation"), 1);
    assert_eq!(command_count(&first, "log"), 1);

    app.pull_request_checks_loading = false;
    app.pull_request_conversation_loading = false;
    app.pull_request_check_log_loading = false;
    app.pull_request_loading = false;
    let mut second = Vec::new();
    app.refresh_pull_request_live(start + PULL_REQUEST_ACTIVE_POLL, false, &mut second);

    assert_eq!(command_count(&second, "checks"), 1);
    assert_eq!(
        command_count(&second, "conversation"),
        0,
        "the conversation holds its own floor rather than following the tick"
    );
    assert_eq!(command_count(&second, "log"), 0);

    let forced = app.webhook_delivered(start + PULL_REQUEST_ACTIVE_POLL);
    assert_eq!(command_count(&forced, "conversation"), 1);
    assert_eq!(command_count(&forced, "log"), 1);
}

#[test]
fn steps_are_selected_with_the_same_keys_as_every_other_list() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.focus = Focus::Content;
    app.pull_request = Some(pull_request(8, "Running", "acme/widget"));
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
    app.pull_request_check_cursor = Some(0);
    let step = |number: usize| crate::git::github::CheckStep {
        number,
        name: format!("step {number}"),
        status: PullRequestCheckStatus::Passed,
        conclusion: "success".to_owned(),
        started_at: String::new(),
        completed_at: String::new(),
        lines: Vec::new(),
    };
    app.pull_request_check_log = Some(CheckRunLog {
        steps: vec![step(1), step(2), step(3)],
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    });
    app.pull_request_step_cursor = 1;

    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 2);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 3);
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 3, "the last step is the end");
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 2);
    assert_eq!(
        app.content_scroll, 0,
        "selecting a step never scrolls the pane behind the reader's back"
    );

    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), now);
    assert!(app.check_step_expanded(2));

    app.geometry.content = Rect::new(0, 0, 80, 20);
    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 2);
    assert!(app.content_scroll > 0);

    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 3);
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 1);
}

#[test]
fn walking_the_check_list_with_keys_drops_the_log_it_walked_away_from() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.focus = Focus::Sidebar;
    app.pull_request = Some(pull_request(8, "Two checks", "acme/widget"));
    app.pull_request_checks = vec![
        check("Build every workspace", PullRequestCheckStatus::Passed),
        check("No-comment policy", PullRequestCheckStatus::Passed),
    ];
    app.pull_request_check_cursor = Some(0);
    app.pull_request_check_log_target = Some(app.pull_request_checks[0].identity());
    app.pull_request_check_log = Some(CheckRunLog {
        steps: vec![crate::git::github::CheckStep {
            number: 1,
            name: "Build every workspace".to_owned(),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: String::new(),
            completed_at: String::new(),
            lines: Vec::new(),
        }],
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    });
    let stale = app.pull_request_check_log_generation;

    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);

    assert_eq!(app.pull_request_check_cursor, Some(1));
    assert!(
        app.pull_request_check_log.is_none(),
        "the header moved to another run, so its body cannot still be the old one"
    );
    assert_ne!(
        app.pull_request_check_log_generation, stale,
        "a reply already in flight for the previous run is invalidated"
    );
}
