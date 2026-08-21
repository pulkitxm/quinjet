use super::*;

#[test]
fn check_status_sections_group_fold_and_navigate() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.focus = Focus::Sidebar;
    app.pull_request = Some(pull_request(8, "Grouped checks", "acme/widget"));
    app.pull_request_checks = vec![
        check("pending", PullRequestCheckStatus::Pending),
        check("broken", PullRequestCheckStatus::Failed),
        check("green", PullRequestCheckStatus::Passed),
        check("skipped", PullRequestCheckStatus::Skipped),
    ];

    let rows = app.check_list_rows();
    assert!(matches!(rows.first(), Some(CheckListRow::Conversation)));
    assert!(rows.iter().any(|row| matches!(row, CheckListRow::Heading)));
    assert!(rows.iter().any(|row| matches!(
        row,
        CheckListRow::Section {
            section: CheckStatusSection::Failed,
            count: 1,
            collapsed: false,
        }
    )));
    assert!(rows.iter().any(|row| matches!(
        row,
        CheckListRow::Section {
            section: CheckStatusSection::InProgress,
            count: 1,
            collapsed: false,
        }
    )));
    assert!(rows.iter().any(|row| matches!(
        row,
        CheckListRow::Section {
            section: CheckStatusSection::Successful,
            count: 1,
            collapsed: false,
        }
    )));
    assert!(rows.iter().any(|row| matches!(
        row,
        CheckListRow::Section {
            section: CheckStatusSection::Skipped,
            count: 1,
            collapsed: false,
        }
    )));

    app.navigate(1, now);
    assert_eq!(app.selected_check_section, Some(CheckStatusSection::Failed));
    assert!(app.pull_request_check_cursor.is_none());

    assert!(app.toggle_selected_check_section());
    assert!(
        app.collapsed_check_sections
            .contains(&CheckStatusSection::Failed)
    );
    assert!(
        !app.check_list_rows()
            .iter()
            .any(|row| matches!(row, CheckListRow::Check { index: 1 }))
    );

    app.navigate(1, now);
    assert_eq!(
        app.selected_check_section,
        Some(CheckStatusSection::InProgress)
    );
    app.navigate(1, now);
    assert_eq!(app.pull_request_check_cursor, Some(0));
    assert!(app.selected_check_section.is_none());

    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), now);
    assert_eq!(
        app.selected_check_section,
        Some(CheckStatusSection::InProgress)
    );
    assert!(app.pull_request_check_cursor.is_none());

    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_check_cursor, Some(0));

    app.go_to_edge(true, now);
    assert_eq!(app.pull_request_check_cursor, Some(3));
    app.go_to_edge(false, now);
    assert!(app.pull_request_check_cursor.is_none());
    assert!(app.selected_check_section.is_none());
}

#[test]
fn opening_the_repository_picker_does_not_cancel_a_pull_request_lookup() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;

    let mut effects = Vec::new();
    app.request_pull_request_lookup(42, false, false, &mut effects);
    let lookup = app.pull_request_generation;
    assert!(app.pull_request_loading);

    app.open_pull_request_repositories(&mut Vec::new());
    assert_eq!(
        app.pull_request_generation, lookup,
        "repository discovery answers on its own counter"
    );

    app.handle_worker_event(
        WorkerEvent::PullRequestLookup {
            generation: lookup,
            result: Ok(crate::git::github::PullRequestSnapshot {
                repositories: Vec::new(),
                selected_repository: None,
                pull_request: pull_request(42, "Still wanted", "acme/widget"),
                warnings: Vec::new(),
                exact_number: Some(42),
                from_cache: false,
            }),
        },
        now,
    );

    assert!(
        !app.pull_request_loading,
        "the lookup reply is still accepted, so polling is never blocked"
    );
    assert_eq!(
        app.pull_request.as_ref().map(|request| request.number),
        Some(42)
    );
}

#[test]
fn moving_to_another_check_never_shows_the_previous_run_under_its_name() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Two checks", "acme/widget"));
    app.pull_request_checks = vec![
        check("Build every workspace", PullRequestCheckStatus::Passed),
        check("No-comment policy", PullRequestCheckStatus::Passed),
    ];

    let mut effects = Vec::new();
    app.select_pull_request_check(Some(0), &mut effects);
    assert_eq!(effects.len(), 1);
    let slow = app.pull_request_check_log_generation;
    assert!(app.pull_request_check_log_loading);

    let mut effects = Vec::new();
    app.select_pull_request_check(Some(1), &mut effects);
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadCheckRunLog { check, .. }
                    if check.name == "No-comment policy"
            )
        ),
        "the newly selected run is requested rather than waited for"
    );
    assert_ne!(
        app.pull_request_check_log_generation, slow,
        "the in-flight read is invalidated by the move"
    );

    let effects = app.handle_worker_event(
        WorkerEvent::CheckRunLog {
            generation: slow,
            result: Ok(CheckRunLog {
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
            }),
        },
        now,
    );

    assert!(effects.is_empty());
    assert!(
        app.pull_request_check_log.is_none(),
        "a reply for the previous check is discarded, not shown under the new one"
    );
    assert!(
        app.pull_request_check_log_loading,
        "the new read is still pending"
    );
}

#[test]
fn a_running_log_follows_its_own_tail_unless_the_reader_scrolled_up() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Running", "acme/widget"));
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Pending)];
    app.pull_request_check_cursor = Some(0);
    app.pull_request_check_log_generation = 3;
    let log = || CheckRunLog {
        steps: Vec::new(),
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    };

    app.content_at_bottom = true;
    app.content_scroll = 120;
    app.handle_worker_event(
        WorkerEvent::CheckRunLog {
            generation: 3,
            result: Ok(log()),
        },
        now,
    );
    assert_eq!(
        app.content_scroll,
        usize::MAX,
        "the draw clamps this to the new end"
    );

    app.content_at_bottom = false;
    app.content_scroll = 40;
    app.pull_request_check_log_generation = 4;
    app.handle_worker_event(
        WorkerEvent::CheckRunLog {
            generation: 4,
            result: Ok(log()),
        },
        now,
    );
    assert_eq!(app.content_scroll, 40);

    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
    app.content_at_bottom = true;
    app.content_scroll = 40;
    app.pull_request_check_log_generation = 5;
    app.handle_worker_event(
        WorkerEvent::CheckRunLog {
            generation: 5,
            result: Ok(log()),
        },
        now,
    );
    assert_eq!(app.content_scroll, 40);
}

#[test]
fn a_background_check_refresh_does_not_reset_another_views_scroll() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    let mut first = check("first", PullRequestCheckStatus::Passed);
    first.link = "https://github.com/acme/widget/actions/runs/1/job/1".to_owned();
    let mut selected = check("selected", PullRequestCheckStatus::Passed);
    selected.link = "https://github.com/acme/widget/actions/runs/1/job/2".to_owned();
    app.pull_request_checks = vec![first, selected.clone()];
    app.pull_request_check_cursor = Some(1);
    app.pull_request_checks_generation = 3;
    app.content_scroll = 18;
    app.horizontal_scroll = 4;
    app.switch_view(View::History, &mut Vec::new());
    app.content_scroll = 51;
    app.horizontal_scroll = 9;

    app.handle_worker_event(
        WorkerEvent::PullRequestChecks {
            generation: 3,
            result: Ok(crate::git::github::PullRequestChecks {
                checks: vec![selected],
                from_cache: false,
            }),
        },
        now,
    );

    assert_eq!(app.pull_request_check_cursor, Some(0));
    assert_eq!(app.content_scroll, 51);
    assert_eq!(app.horizontal_scroll, 9);

    app.switch_view(View::PullRequests, &mut Vec::new());
    assert_eq!(app.content_scroll, 0);
    assert_eq!(app.horizontal_scroll, 0);
}

#[test]
fn a_settled_pull_request_polls_less_often_than_a_running_one() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Checks", "acme/widget"));
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
    assert_eq!(app.pull_request_poll_interval(), PULL_REQUEST_IDLE_POLL);

    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Pending)];
    assert_eq!(app.pull_request_poll_interval(), PULL_REQUEST_ACTIVE_POLL);

    app.view = View::Changes;
    assert_eq!(
        app.pull_request_poll_interval(),
        PULL_REQUEST_BACKGROUND_POLL,
        "a pull request nobody is looking at still stays fresh, just cheaply"
    );
}

#[test]
fn a_moved_head_reindexes_the_diff_and_keeps_everything_else() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request_generation = 4;
    app.pull_request_loading = true;
    app.pull_request = Some(pull_request(8, "Force pushed", "acme/widget"));
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_workspace_generation = Some(2);
    app.pull_request_documents
        .insert(PathBuf::from("src/one.rs"), DiffDocument::default());
    app.pull_request_check_cursor = Some(0);
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];

    let mut moved = pull_request(8, "Force pushed", "acme/widget");
    moved.head_oid = "rewritten".to_owned();
    let repository = moved.base_repository.clone();
    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestLookup {
            generation: 4,
            result: Ok(crate::git::github::PullRequestSnapshot {
                repositories: vec![repository.clone()],
                selected_repository: Some(repository),
                pull_request: moved,
                warnings: Vec::new(),
                exact_number: Some(8),
                from_cache: false,
            }),
        },
        now,
    );

    assert!(app.pull_request_workspace_generation.is_none());
    assert!(app.pull_request_documents.is_empty());
    assert_eq!(
        app.pull_request_section,
        PullRequestSection::Files,
        "a force push replaces the diff, not the reader's place in the view"
    );
    assert_eq!(app.pull_request_check_cursor, Some(0));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::PreparePullRequest { .. })
    )));
}

#[test]
fn a_forwarded_webhook_refreshes_immediately_instead_of_waiting_for_the_poll() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Checks", "acme/widget"));
    app.pull_request_exact_number = Some(8);
    app.pull_request_poll_due = Some(now + Duration::from_secs(3_600));

    let effects = app.webhook_delivered(now);

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestChecks { .. })
    )));
    assert!(
        app.pull_request_poll_due
            .is_some_and(|due| due <= now + PULL_REQUEST_IDLE_POLL),
        "the delivery also restarts the poll clock"
    );
}

#[test]
fn history_reset_requested_during_a_load_runs_after_the_in_flight_page() {
    let mut app = App::new("/tmp/repo", "repo");
    app.history_generation = 4;
    app.history_loading = true;
    let mut effects = Vec::new();

    app.request_history(true, &mut effects);
    assert!(effects.is_empty());
    assert!(app.history_refresh_again);

    let effects = app.handle_worker_event(
        WorkerEvent::History {
            generation: 4,
            skip: 0,
            result: Ok(Vec::new()),
        },
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadHistory {
                generation: 5,
                skip: 0,
                limit: HISTORY_PAGE_SIZE,
                ..
            }
        )
    ));
    assert!(app.history_loading);
    assert!(!app.history_refresh_again);
}
