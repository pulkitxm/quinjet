use super::*;

#[test]
fn each_view_restores_its_document_filter_and_viewport() {
    let mut app = app_with_changes();
    let changes_document = indexed_document(&["src/main.rs", "README.md"]);
    app.document = changes_document.clone();
    app.focus = Focus::Content;
    app.filter = "main".to_owned();
    app.sidebar_offset = 4;
    app.sidebar_free_scroll = true;
    app.sidebar_last_cursor = Some(2);
    app.content_scroll = 17;
    app.horizontal_scroll = 9;
    app.selected_preview_file = Some(PathBuf::from("README.md"));
    app.preview_file_cursor = 1;
    app.collapsed_preview_files = HashSet::from([PathBuf::from("src/main.rs")]);

    app.switch_view(View::History, &mut Vec::new());
    let history_document = DiffDocument::empty("History preview", "history body");
    app.document = history_document.clone();
    app.focus = Focus::Sidebar;
    app.filter = "release".to_owned();
    app.sidebar_offset = 8;
    app.sidebar_free_scroll = true;
    app.sidebar_last_cursor = Some(6);
    app.content_scroll = 31;
    app.horizontal_scroll = 2;
    app.selected_preview_file = Some(PathBuf::from("CHANGELOG.md"));
    app.preview_file_cursor = 3;
    app.expanded_preview_files = HashSet::from([PathBuf::from("CHANGELOG.md")]);

    app.switch_view(View::PullRequests, &mut Vec::new());
    app.switch_view(View::Changes, &mut Vec::new());

    assert_eq!(app.document, changes_document);
    assert_eq!(app.focus, Focus::Content);
    assert_eq!(app.filter, "main");
    assert_eq!(app.sidebar_offset, 4);
    assert!(app.sidebar_free_scroll);
    assert_eq!(app.sidebar_last_cursor, Some(2));
    assert_eq!(app.content_scroll, 17);
    assert_eq!(app.horizontal_scroll, 9);
    assert_eq!(app.selected_preview_file, Some(PathBuf::from("README.md")));
    assert_eq!(app.preview_file_cursor, 1);
    assert_eq!(
        app.collapsed_preview_files,
        HashSet::from([PathBuf::from("src/main.rs")])
    );

    app.switch_view(View::History, &mut Vec::new());

    assert_eq!(app.document, history_document);
    assert_eq!(app.focus, Focus::Sidebar);
    assert_eq!(app.filter, "release");
    assert_eq!(app.sidebar_offset, 8);
    assert!(app.sidebar_free_scroll);
    assert_eq!(app.sidebar_last_cursor, Some(6));
    assert_eq!(app.content_scroll, 31);
    assert_eq!(app.horizontal_scroll, 2);
    assert_eq!(
        app.selected_preview_file,
        Some(PathBuf::from("CHANGELOG.md"))
    );
    assert_eq!(app.preview_file_cursor, 3);
    assert_eq!(
        app.expanded_preview_files,
        HashSet::from([PathBuf::from("CHANGELOG.md")])
    );
}

#[test]
fn pull_request_diff_position_survives_outer_view_switches() {
    let mut app = App::new("/tmp/repo", "repo");
    app.switch_view(View::PullRequests, &mut Vec::new());
    app.pull_request = Some(pull_request(42, "Keep position", "acme/widget"));
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_file_view = PullRequestFileView::SingleFile;
    app.pull_request_file_cursor = 7;
    app.pull_request_tree_cursor = 9;
    app.collapsed_pull_request_directories = HashSet::from([PathBuf::from("src/generated")]);
    app.document = indexed_document(&["src/main.rs"]);
    app.content_scroll = 240;
    app.horizontal_scroll = 18;
    app.sidebar_offset = 5;
    app.sidebar_free_scroll = true;

    app.switch_view(View::History, &mut Vec::new());
    app.content_scroll = 11;
    app.switch_view(View::PullRequests, &mut Vec::new());

    assert_eq!(app.pull_request_section, PullRequestSection::Files);
    assert_eq!(app.pull_request_file_view, PullRequestFileView::SingleFile);
    assert_eq!(app.pull_request_file_cursor, 7);
    assert_eq!(app.pull_request_tree_cursor, 9);
    assert_eq!(
        app.collapsed_pull_request_directories,
        HashSet::from([PathBuf::from("src/generated")])
    );
    assert_eq!(app.content_scroll, 240);
    assert_eq!(app.horizontal_scroll, 18);
    assert_eq!(app.sidebar_offset, 5);
    assert!(app.sidebar_free_scroll);
}

#[test]
fn interrupted_local_preview_resumes_without_losing_visible_state() {
    let mut app = app_with_changes();
    let path = PathBuf::from("src/main.rs");
    app.document = indexed_document(&["src/main.rs"]);
    app.document_loading = true;
    app.local_diff_request = app.local_diff_request_for_view();
    app.local_diff_workspace_generation = Some(3);
    app.local_diff_index = Some(DiffIndex {
        title: "Working Tree".to_owned(),
        files: vec![crate::git::diff::DiffFileIndexEntry {
            path: path.clone(),
            old_path: None,
            status: "modified".to_owned(),
            counts: None,
        }],
        truncated: false,
        commit_details: None,
    });
    app.local_diff_loading_path = Some(path.clone());
    app.content_scroll = 12;

    app.switch_view(View::History, &mut Vec::new());
    let mut effects = Vec::new();
    app.switch_view(View::Changes, &mut effects);

    assert_eq!(app.content_scroll, 12);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile {
                workspace_generation: 3,
                path: queued,
                ..
            } if queued == &path
        )
    ));
}
