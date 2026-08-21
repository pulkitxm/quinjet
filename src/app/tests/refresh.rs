use super::*;

#[test]
fn scheduling_a_new_selection_immediately_invalidates_an_in_flight_preview() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 7;
    app.document = DiffDocument::empty("Current", "keep me");
    app.local_diff_workspace_generation = Some(7);
    app.local_diff_index = Some(DiffIndex {
        title: "Previous commit".to_owned(),
        files: vec![crate::git::diff::DiffFileIndexEntry {
            path: PathBuf::from("stale.rs"),
            old_path: None,
            status: "modified".to_owned(),
            counts: None,
        }],
        truncated: false,
        commit_details: None,
    });

    let now = Instant::now();
    app.schedule_preview(now);
    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), now);
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: 7,
            workspace_generation: 7,
            path: PathBuf::from("stale.rs"),
            result: Ok(DiffDocument::empty("Stale", "replace me")),
        },
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert_eq!(app.diff_generation, 8);
    assert!(app.local_diff_index.is_none());
    assert!(app.local_diff_workspace_generation.is_none());
    assert_eq!(app.document.title, "Working Tree");
    assert_eq!(app.document.lines[0].text(), "Loading selected changes…");
}

#[test]
fn switching_views_replaces_stale_preview_before_async_work_completes() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.pull_request = Some(pull_request(6, "Slow preview", "acme/widget"));
    app.view = View::PullRequests;
    app.document = DiffDocument::empty("PR #6", "stale PR contents");
    app.history.push(Commit {
        id: "a".repeat(40),
        short_id: "aaaaaaa".to_owned(),
        parent_ids: Vec::new(),
        author: String::new(),
        author_email: String::new(),
        authored_at: String::new(),
        committer: String::new(),
        committer_email: String::new(),
        committed_at: String::new(),
        relative_date: String::new(),
        subject: "Selected history commit".to_owned(),
        decorations: Vec::new(),
    });

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), now);

    assert!(effects.is_empty());
    assert_eq!(app.view, View::History);
    assert_eq!(app.document.title, "aaaaaaa — Selected history commit");
    assert_eq!(app.document.lines[0].text(), "Loading commit preview…");
    assert!(app.preview_due.is_some());
}

#[test]
fn switching_views_invalidates_an_in_flight_pull_request_preview() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(6, "Slow preview", "acme/widget"));
    app.diff_generation = 9;

    app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), now);
    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestDiff {
            generation: 9,
            result: Ok(DiffDocument::empty("PR #6", "stale PR contents")),
        },
        now,
    );

    assert!(effects.is_empty());
    assert_eq!(app.view, View::History);
    assert_eq!(app.document.title, "Commit History");
    assert_eq!(
        app.document.lines[0].text(),
        "No commits in this repository"
    );
    assert_ne!(app.document.lines[0].text(), "stale PR contents");
}

#[test]
fn stale_pull_request_metadata_does_not_replace_the_active_lookup() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_generation = 2;
    app.pull_request_loading = true;

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestLookup {
            generation: 1,
            result: Ok(crate::git::github::PullRequestSnapshot {
                repositories: Vec::new(),
                selected_repository: None,
                pull_request: pull_request(1, "Stale", "acme/widget"),
                warnings: Vec::new(),
                exact_number: Some(1),
                from_cache: false,
            }),
        },
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert!(app.pull_request_loading);
    assert!(app.pull_request.is_none());
}

#[test]
fn exact_pull_request_lookup_accepts_only_digits_and_keeps_repository_scope() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    let repository = pull_request(1, "One", "acme/widget").base_repository;
    app.github_repositories = vec![repository.clone()];
    app.pull_request_repository = Some(repository.clone());
    let now = Instant::now();

    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
    for character in ['1', '2', 'x'] {
        app.handle_key(
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
            now,
        );
    }
    app.handle_paste("abc3def");
    assert_eq!(app.pull_request_lookup.value, "123");

    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LookupPullRequest {
                number: 123,
                repository: Some(selected),
                ..
            } if selected.url == repository.url
        )
    ));
    assert!(!app.pull_request_lookup_active);
}

#[test]
fn history_branch_picker_changes_only_the_viewed_revision() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::History;
    app.status.branch.head = "main".to_owned();
    let now = Instant::now();

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadHistoryBranches { generation: 1 }
        )
    ));
    app.handle_worker_event(
        WorkerEvent::HistoryBranches {
            generation: 1,
            result: Ok(vec![
                HistoryBranch {
                    name: "main".to_owned(),
                    reference: "refs/heads/main".to_owned(),
                    current: true,
                    remote: false,
                    relative_date: "now".to_owned(),
                    short_id: "aaaaaaa".to_owned(),
                },
                HistoryBranch {
                    name: "topic".to_owned(),
                    reference: "refs/heads/topic".to_owned(),
                    current: false,
                    remote: false,
                    relative_date: "now".to_owned(),
                    short_id: "bbbbbbb".to_owned(),
                },
            ]),
        },
        now,
    );
    let Some(Modal::HistoryBranches { selected, .. }) = app.modal.as_mut() else {
        panic!("expected history branch picker");
    };
    *selected = 1;

    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert_eq!(app.status.branch.head, "main");
    assert_eq!(app.history_branch_label(), "topic");
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadHistory { revision, skip: 0, .. }
                if revision == "refs/heads/topic"
        )
    ));
}

#[test]
fn collapse_preference_survives_documents_selections_and_views() {
    let mut app = app_with_changes();
    app.document = indexed_document(&["src/main.rs", "README.md"]);
    let now = Instant::now();

    app.handle_key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT), now);
    assert!(app.files_collapsed);
    app.toggle_preview_file(PathBuf::from("src/main.rs"), &mut Vec::new());
    assert!(
        app.files_collapsed,
        "a one-file override must not reset the preference"
    );
    assert!(!app.preview_file_collapsed("src/main.rs"));

    app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), now);
    assert!(app.files_collapsed);
    assert!(app.expanded_preview_files.is_empty());

    app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE), now);
    app.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE), now);
    assert!(app.files_collapsed);
}

#[test]
fn branch_dialog_renames_the_selected_local_branch() {
    let mut app = App::new("/tmp/repo", "repo");
    app.modal = Some(Modal::Branches {
        items: vec![Branch {
            name: "topic".to_owned(),
            current: false,
            upstream: Some("origin/topic".to_owned()),
            relative_date: "now".to_owned(),
            short_id: "abc1234".to_owned(),
        }],
        selected: 0,
        query: TextBuffer::default(),
        loading: false,
    });

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(effects.is_empty());
    let Some(Modal::Prompt { input, kind, .. }) = app.modal.as_mut() else {
        panic!("expected rename prompt");
    };
    assert_eq!(input.value, "topic");
    assert!(matches!(kind, PromptKind::RenameBranch { old } if old == "topic"));
    *input = TextBuffer::new("feature/topic");

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)]
            if matches!(command.as_ref(), WorkerCommand::Operate {
                operation: GitOperation::RenameBranch { old, new }, ..
            } if old == "topic" && new == "feature/topic")
    ));
    assert_eq!(app.busy.as_deref(), Some("Renaming branch"));
}

#[test]
fn long_running_git_operations_animate_until_completion() {
    let mut app = App::new("/tmp/repo", "repo");
    let mut effects = Vec::new();
    app.queue_operation(GitOperation::Pull, &mut effects);
    let initial = app.operation_spinner();

    let (_, changed) = app.tick(Instant::now());

    assert!(changed);
    assert_ne!(app.operation_spinner(), initial);
    assert_eq!(app.busy.as_deref(), Some("Pulling changes"));
}

#[test]
fn compare_branch_picker_queues_a_head_diff_without_checkout() {
    let mut app = app_with_changes();
    app.history_branches_loaded = true;
    app.history_branches = vec![
        HistoryBranch {
            name: "main".to_owned(),
            reference: "refs/heads/main".to_owned(),
            current: true,
            remote: false,
            relative_date: "now".to_owned(),
            short_id: "aaaaaaa".to_owned(),
        },
        HistoryBranch {
            name: "topic".to_owned(),
            reference: "refs/heads/topic".to_owned(),
            current: false,
            remote: false,
            relative_date: "now".to_owned(),
            short_id: "bbbbbbb".to_owned(),
        },
    ];
    let now = Instant::now();

    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE), now)
            .is_empty()
    );
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::PrepareLocalDiff { request, .. }
                if matches!(request.as_ref(), LocalDiffRequest::Branch { branch, .. } if branch.name == "topic")
        )
    ));
    assert!(matches!(
        app.auxiliary_preview,
        Some(AuxiliaryPreview::Branch(ref branch)) if branch.name == "topic"
    ));
    assert_eq!(app.focus, Focus::Content);
}
