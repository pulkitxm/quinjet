use super::*;

#[test]
fn an_existing_expand_all_preference_loads_every_file_in_a_new_index() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 9;
    app.files_collapsed = false;
    app.collapse_preference_set = true;

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: 9,
            result: Ok(DiffIndex {
                title: "Next commit".to_owned(),
                files: ["one.rs", "two.rs", "three.rs"]
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

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile { path, .. } if path == Path::new("one.rs")
        )
    ));
    assert_eq!(
        app.local_diff_pending_paths,
        VecDeque::from([PathBuf::from("two.rs"), PathBuf::from("three.rs")])
    );
    assert!(
        app.preview_file_paths()
            .iter()
            .all(|path| !app.preview_file_collapsed(&path.to_string_lossy()))
    );
}

#[test]
fn local_diff_file_replies_must_match_the_prepared_workspace() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 12;
    app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: 12,
            result: Ok(DiffIndex {
                title: "Current".to_owned(),
                files: vec![crate::git::diff::DiffFileIndexEntry {
                    path: PathBuf::from("same.rs"),
                    old_path: None,
                    status: "modified".to_owned(),
                    counts: None,
                }],
                truncated: false,
                commit_details: None,
            }),
        },
        Instant::now(),
    );

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: 12,
            workspace_generation: 11,
            path: PathBuf::from("same.rs"),
            result: Ok(DiffDocument::empty("Old commit", "stale")),
        },
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert_eq!(app.local_diff_loading_path, Some(PathBuf::from("same.rs")));
    assert!(!app.local_diff_single_loaded);
    assert_ne!(app.document.title, "Old commit");
}

#[test]
fn local_diff_failure_replaces_the_loading_placeholder() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 12;
    app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: 12,
            result: Ok(DiffIndex {
                title: "Stash".to_owned(),
                files: vec![crate::git::diff::DiffFileIndexEntry {
                    path: PathBuf::from("index2.ts"),
                    old_path: None,
                    status: "added".to_owned(),
                    counts: None,
                }],
                truncated: false,
                commit_details: None,
            }),
        },
        Instant::now(),
    );

    app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: 12,
            workspace_generation: 12,
            path: PathBuf::from("index2.ts"),
            result: Err("missing stash parent".to_owned()),
        },
        Instant::now(),
    );

    assert!(app.local_diff_single_loaded);
    assert!(
        app.document
            .lines
            .iter()
            .any(|line| line.text() == "Unable to load diff: missing stash parent")
    );
}

#[test]
fn pull_request_folders_are_selectable_and_collapsible() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_section = PullRequestSection::Files;
    app.focus = Focus::Sidebar;
    app.pull_request_files = ["src/app.rs", "src/git/diff.rs", "tests/ui.rs"]
        .into_iter()
        .map(|path| PullRequestFile {
            path: PathBuf::from(path),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        })
        .collect();
    app.sync_pull_request_tree_cursor_to_file();

    let cursor = app.pull_request_tree_cursor;
    let entries = app.pull_request_tree_entries();
    assert!(matches!(
        entries.get(cursor),
        Some(PullRequestTreeEntry::File { index: 0, .. })
    ));

    app.handle_key(
        KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        Instant::now(),
    );
    let cursor = app.pull_request_tree_cursor;
    assert!(matches!(
        app.pull_request_tree_entries().get(cursor),
        Some(PullRequestTreeEntry::Directory { path, .. }) if path == Path::new("src")
    ));

    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        Instant::now(),
    );
    assert_eq!(app.focus, Focus::Content);
    assert!(!app.pull_request_directory_collapsed(Path::new("src")));

    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        Instant::now(),
    );
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(app.pull_request_directory_collapsed(Path::new("src")));
    assert_eq!(
        app.pull_request_tree_entries()
            .iter()
            .filter(|entry| matches!(entry, PullRequestTreeEntry::File { .. }))
            .count(),
        1
    );

    app.handle_key(
        KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(app.pull_request_directory_collapsed(Path::new("src")));

    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(!app.pull_request_directory_collapsed(Path::new("src")));
    assert_eq!(
        app.pull_request_tree_entries()
            .iter()
            .filter(|entry| matches!(entry, PullRequestTreeEntry::File { .. }))
            .count(),
        3
    );

    app.geometry.sidebar = Rect::new(0, 0, 40, 10);
    app.geometry.sidebar_hits = vec![SidebarHitArea {
        area: Rect::new(0, 2, 40, 1),
        target: SidebarHit::PullRequestDirectory(PathBuf::from("src/git")),
    }];
    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        Instant::now(),
    );
    assert!(!app.pull_request_directory_collapsed(Path::new("src/git")));

    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );
    assert!(app.pull_request_directory_collapsed(Path::new("src/git")));
    assert!(
        !app.pull_request_tree_entries()
            .iter()
            .any(|entry| { matches!(entry, PullRequestTreeEntry::File { index: 1, .. }) })
    );
}

#[test]
fn pull_request_tree_compacts_single_child_directory_chains() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_files = [
        "apps/web/app/one.txt",
        "apps/web/modules/marketing/landing/two.txt",
    ]
    .into_iter()
    .map(|path| PullRequestFile {
        path: PathBuf::from(path),
        old_path: None,
        status: PullRequestFileStatus::Modified,
        counts: None,
    })
    .collect();

    let entries = app.pull_request_tree_entries();

    assert!(matches!(
        entries.first(),
        Some(PullRequestTreeEntry::Directory { path, label, depth: 0 })
            if path == Path::new("apps/web") && label == "apps/web"
    ));
    assert!(entries.iter().any(|entry| matches!(
        entry,
        PullRequestTreeEntry::Directory { path, label, depth: 1 }
            if path == Path::new("apps/web/modules/marketing/landing")
                && label == "modules/marketing/landing"
    )));

    app.toggle_pull_request_directory(PathBuf::from("apps/web"));
    assert_eq!(app.pull_request_tree_entries().len(), 1);
}

#[test]
fn pull_request_defaults_to_all_files_then_files_tab_restores_it_from_single_file() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_section = PullRequestSection::Files;
    app.focus = Focus::Sidebar;
    app.pull_request = Some(pull_request(8, "Large change", "acme/widget"));
    app.diff_generation = 10;
    app.pull_request_diff_source = Some(PullRequestDiffSource::PullRequest);
    let files = ["src/first.rs", "src/second.rs"]
        .into_iter()
        .map(|path| PullRequestFile {
            path: PathBuf::from(path),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        })
        .collect();
    let now = Instant::now();

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestIndex {
            generation: 10,
            result: Ok(PullRequestDiffIndex {
                files,
                total_files: 2,
                truncated: false,
            }),
        },
        now,
    );

    assert_eq!(app.pull_request_file_view, PullRequestFileView::AllFiles);
    assert_eq!(app.document.file_count(), 2);
    assert!(app.preview_files_all_collapsed());
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch {
                    workspace_generation: 10,
                    paths,
                } if paths == &[PathBuf::from("src/first.rs"), PathBuf::from("src/second.rs")]
            )
        ),
        "the whole index is fetched in one batch rather than a file at a time"
    );

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestDiffBatch {
            workspace_generation: 10,
            result: Ok(vec![
                (
                    PathBuf::from("src/first.rs"),
                    indexed_document(&["src/first.rs"]),
                ),
                (
                    PathBuf::from("src/second.rs"),
                    indexed_document(&["src/second.rs"]),
                ),
            ]),
        },
        now,
    );
    assert!(effects.is_empty(), "no file is left to fetch");
    assert_eq!(app.document.file_count(), 2);

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_file_cursor, 1);
    assert_eq!(app.pull_request_file_view, PullRequestFileView::SingleFile);
    let (effects, _) = app.tick(now + PREVIEW_DEBOUNCE);
    assert!(
        effects.is_empty(),
        "a prefetched file opens without another Git round trip"
    );
    assert_eq!(app.document.file_count(), 1);
    assert!(!app.preview_files_collapsible());

    app.geometry.sidebar = Rect::new(0, 0, 20, 10);
    app.geometry.sidebar_hits = vec![SidebarHitArea {
        area: Rect::new(1, 1, 8, 1),
        target: SidebarHit::PullRequestFiles,
    }];
    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 1,
            modifiers: KeyModifiers::NONE,
        },
        now,
    );

    assert!(
        !effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command)
                if matches!(command.as_ref(), WorkerCommand::LoadPullRequestFile { .. })
        )),
        "the prefetched first file is cached"
    );
    assert_eq!(app.pull_request_file_view, PullRequestFileView::AllFiles);
    assert_eq!(app.document.file_count(), 2);
    assert!(app.preview_files_all_collapsed());
}

#[test]
fn sidebar_wheel_scroll_pans_without_moving_the_selection() {
    let mut app = App::new("/tmp/repo", "repo");
    app.geometry.sidebar = Rect::new(0, 0, 30, 10);
    app.sidebar_viewport(0, 5, 40);
    let cursor_before = app.history_cursor;

    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );
    assert!(effects.is_empty(), "panning requests no preview");
    assert_eq!(app.history_cursor, cursor_before, "the selection stays put");
    assert_eq!(app.sidebar_offset, 2, "the viewport pans");
    assert!(app.sidebar_free_scroll, "the window detaches");

    app.sidebar_viewport(0, 5, 40);
    assert_eq!(
        app.sidebar_offset, 2,
        "an unmoved selection does not snap the window back"
    );

    app.sidebar_viewport(1, 5, 40);
    assert!(!app.sidebar_free_scroll, "a selection change reattaches");
    assert_eq!(app.sidebar_offset, 1, "the window follows the selection");
}

#[test]
fn sidebar_wheel_scroll_loads_more_history_near_the_end() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::History;
    app.geometry.sidebar = Rect::new(0, 0, 30, 10);
    app.history = vec![
        Commit {
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
            subject: "Commit".to_owned(),
            decorations: Vec::new(),
        };
        HISTORY_PAGE_SIZE
    ];
    app.sidebar_offset = HISTORY_PAGE_SIZE - 31;

    let first = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );
    assert!(
        first.is_empty(),
        "pagination waits until the viewport nears the end"
    );

    let second = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );
    assert!(matches!(
        second.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadHistory {
                skip: HISTORY_PAGE_SIZE,
                limit: HISTORY_PAGE_SIZE,
                ..
            }
        )
    ));
    assert_eq!(app.history_cursor, 0, "the selected commit stays put");
    assert!(app.history_loading);
}
