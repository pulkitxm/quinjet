use super::*;

#[test]
fn single_file_views_cannot_be_collapsed() {
    let mut app = App::new("/tmp/repo", "repo");
    app.document = indexed_document(&["src/main.rs"]);
    app.selected_preview_file = Some(PathBuf::from("src/main.rs"));
    let now = Instant::now();

    assert!(!app.preview_files_collapsible());
    assert!(!app.preview_file_collapsed("src/main.rs"));
    assert!(!app.preview_files_all_collapsed());

    app.handle_key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT), now);
    app.toggle_preview_file(PathBuf::from("src/main.rs"), &mut Vec::new());

    assert!(!app.files_collapsed);
    assert!(app.collapsed_preview_files.is_empty());
    assert!(app.expanded_preview_files.is_empty());
    assert!(!app.preview_file_collapsed("src/main.rs"));
}

#[test]
fn preserves_selected_change_across_status_refresh_order() {
    let mut app = app_with_changes();
    app.change_cursor = 1;
    let selected = app.selected_change().cloned();
    app.status.changes.swap(0, 1);
    app.restore_change_selection(selected.as_ref());
    assert_eq!(
        app.selected_change().unwrap().path,
        PathBuf::from("README.md")
    );
}

#[test]
fn startup_does_not_fetch_any_pull_request_data() {
    let mut app = App::new("/tmp/repo", "repo");

    let effects = app.initial_effects();

    assert!(matches!(
        effects.as_slice(),
        [
            AppEffect::Git(refresh),
            AppEffect::Git(worktrees),
            AppEffect::Git(history),
            AppEffect::Git(branches),
            AppEffect::Git(repository),
        ] if matches!(refresh.as_ref(), WorkerCommand::Refresh { .. })
            && matches!(worktrees.as_ref(), WorkerCommand::LoadWorktrees { .. })
            && matches!(history.as_ref(), WorkerCommand::LoadHistory { .. })
            && matches!(branches.as_ref(), WorkerCommand::LoadHistoryBranches { .. })
            && matches!(repository.as_ref(), WorkerCommand::LoadLocalGitHubRepository)
    ));
    assert!(!app.pull_request_loading);
    assert!(app.pull_request.is_none());
    assert_eq!(app.pull_request_generation, 0);
}

#[test]
fn opening_pr_tab_only_focuses_the_number_field() {
    let mut app = App::new("/tmp/repo", "repo");

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert_eq!(app.view, View::PullRequests);
    assert!(app.pull_request_lookup_active);
    assert_eq!(app.document.title, "Open Pull Request");
}

#[test]
fn recent_pull_requests_open_with_space_while_enter_moves_focus() {
    let mut app = App::new("/tmp/repo", "repo");
    app.recent_pull_requests = vec![
        RecentPullRequest::from(&pull_request(39, "First", "acme/widget")),
        RecentPullRequest::from(&pull_request(42, "Second", "acme/widget")),
    ];
    let now = Instant::now();

    app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE), now);
    assert!(!app.pull_request_lookup_active);
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert!(effects.is_empty());
    assert_eq!(app.focus, Focus::Content);
    assert!(app.pull_request.is_none());

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), now);

    assert_eq!(app.recent_pull_request_cursor, 1);
    assert_eq!(app.pull_request_lookup.value, "42");
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LookupPullRequest {
                number: 42,
                repository: Some(repository),
                ..
            } if repository.name_with_owner == "acme/widget"
        )
    ));
}

#[test]
fn repository_picker_is_also_discovered_only_on_explicit_request() {
    let mut app = App::new("/tmp/repo", "repo");
    app.switch_view(View::PullRequests, &mut Vec::new());

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadGitHubRepositories {
                generation: 1,
                refresh: false,
            }
        )
    ));
    assert!(matches!(
        app.modal,
        Some(Modal::PullRequestRepositories { loading: true, .. })
    ));
}

#[test]
fn numeric_lookup_discovers_the_repository_on_demand() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.switch_view(View::PullRequests, &mut Vec::new());
    for character in ['4', '2'] {
        app.handle_key(
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
            now,
        );
    }

    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LookupPullRequest {
                number: 42,
                repository: None,
                ..
            }
        )
    ));
    assert_eq!(
        app.pull_request_progress,
        Some(PullRequestProgress::LoadingMetadata)
    );
    assert!(app.pull_request_loading);
}

#[test]
fn opening_a_pull_request_prefetches_diffs_checks_and_conversation_from_overview() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_generation = 3;
    app.pull_request_loading = true;
    let request = pull_request(8, "Cross-fork update", "acme/widget");
    let repository = request.base_repository.clone();

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestLookup {
            generation: 3,
            result: Ok(crate::git::github::PullRequestSnapshot {
                repositories: vec![repository.clone()],
                selected_repository: Some(repository),
                pull_request: request,
                warnings: Vec::new(),
                exact_number: Some(8),
                from_cache: false,
            }),
        },
        Instant::now(),
    );

    assert_eq!(effects.len(), 4);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestStack { pull_request, .. }
                if pull_request.number == 8
        )
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestConversation { pull_request, .. }
                if pull_request.number == 8
        )
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(
            command.as_ref(),
            WorkerCommand::PreparePullRequest { pull_request, .. }
                if pull_request.number == 8
        )
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestChecks { pull_request, .. }
                if pull_request.number == 8
        )
    )));
    assert_eq!(
        app.pull_request_progress,
        Some(PullRequestProgress::PreparingRepository)
    );
    assert_eq!(
        app.pull_request_section,
        PullRequestSection::Overview,
        "an opened pull request lands on itself, not on its files"
    );
    let workspace_generation = app.diff_generation;
    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestIndex {
            generation: workspace_generation,
            result: Ok(PullRequestDiffIndex {
                files: ["src/first.rs", "src/second.rs"]
                    .into_iter()
                    .map(|path| PullRequestFile {
                        path: PathBuf::from(path),
                        old_path: None,
                        status: PullRequestFileStatus::Modified,
                        counts: None,
                    })
                    .collect(),
                total_files: 2,
                truncated: false,
            }),
        },
        Instant::now(),
    );
    assert_eq!(app.pull_request_section, PullRequestSection::Overview);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestFileBatch {
                workspace_generation: generation,
                paths,
            } if *generation == workspace_generation
                && paths == &[PathBuf::from("src/first.rs"), PathBuf::from("src/second.rs")]
        )
    ));
    assert_eq!(
        app.pull_request.as_ref().unwrap().description,
        "A detailed pull-request description"
    );
}

#[test]
fn local_diff_index_prefetches_one_file_then_loads_only_an_expanded_path() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 5;
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: 5,
            result: Ok(DiffIndex {
                title: "Branch comparison".to_owned(),
                files: ["src/first.rs", "src/second.rs"]
                    .into_iter()
                    .map(|path| crate::git::diff::DiffFileIndexEntry {
                        path: PathBuf::from(path),
                        old_path: None,
                        status: "modified".to_owned(),
                        counts: None,
                    })
                    .collect(),
                truncated: false,
                commit_details: None,
            }),
        },
        Instant::now(),
    );

    assert_eq!(app.document.file_count(), 2);
    assert!(app.preview_files_all_collapsed());
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile {
                workspace_generation: 5,
                path,
                ..
            } if path == Path::new("src/first.rs")
        )
    ));

    let first_generation = app.diff_generation;
    app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: first_generation,
            workspace_generation: 5,
            path: PathBuf::from("src/first.rs"),
            result: Ok(DiffDocument::empty("first", "loaded")),
        },
        Instant::now(),
    );
    let mut effects = Vec::new();
    app.toggle_preview_file(PathBuf::from("src/second.rs"), &mut effects);

    assert_eq!(app.document.file_count(), 2);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile {
                workspace_generation: 5,
                path,
                ..
            } if path == Path::new("src/second.rs")
        )
    ));
}

#[test]
fn expanding_all_local_files_loads_every_visible_diff_in_order() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 5;
    let now = Instant::now();
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: 5,
            result: Ok(DiffIndex {
                title: "Commit details".to_owned(),
                files: ["src/first.rs", "src/second.rs", "src/third.rs"]
                    .into_iter()
                    .map(|path| crate::git::diff::DiffFileIndexEntry {
                        path: PathBuf::from(path),
                        old_path: None,
                        status: "modified".to_owned(),
                        counts: None,
                    })
                    .collect(),
                truncated: false,
                commit_details: None,
            }),
        },
        now,
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile { path, .. }
                if path == Path::new("src/first.rs")
        )
    ));

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: app.diff_generation,
            workspace_generation: 5,
            path: PathBuf::from("src/first.rs"),
            result: Ok(indexed_document(&["src/first.rs"])),
        },
        now,
    );
    assert!(effects.is_empty());

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile { path, .. }
                if path == Path::new("src/second.rs")
        )
    ));

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: app.diff_generation,
            workspace_generation: 5,
            path: PathBuf::from("src/second.rs"),
            result: Ok(indexed_document(&["src/second.rs"])),
        },
        now,
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile { path, .. }
                if path == Path::new("src/third.rs")
        )
    ));

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: app.diff_generation,
            workspace_generation: 5,
            path: PathBuf::from("src/third.rs"),
            result: Ok(indexed_document(&["src/third.rs"])),
        },
        now,
    );
    assert!(effects.is_empty());
    assert_eq!(app.local_diff_documents.len(), 3);
    assert!(app.local_diff_pending_paths.is_empty());
}
