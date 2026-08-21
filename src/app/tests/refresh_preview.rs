use super::*;

fn loaded_changes_preview() -> (App, PathBuf, DiffIndex, DiffDocument) {
    let mut app = app_with_changes();
    let path = PathBuf::from("src/main.rs");
    let index = DiffIndex {
        title: "Current diff".to_owned(),
        files: vec![crate::git::diff::DiffFileIndexEntry {
            path: path.clone(),
            old_path: None,
            status: "modified".to_owned(),
            counts: None,
        }],
        truncated: false,
        commit_details: None,
    };
    let document = crate::git::diff::parse_diff(
        b"diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+current\n",
        "Current diff",
        Some(&path),
        false,
    );
    app.document = document.clone();
    app.changes_diff_version = 3;
    app.diff_generation = 5;
    app.local_diff_request = Some(LocalDiffRequest::Changes {
        changes: app.selected_change().cloned().into_iter().collect(),
        version: 3,
        expanded: false,
    });
    app.local_diff_workspace_generation = Some(5);
    app.local_diff_index = Some(index.clone());
    app.local_diff_single_loaded = true;
    app.selected_preview_file = Some(path.clone());
    (app, path, index, document)
}

fn parsed_change_document(path: &Path, title: &str, value: &str) -> DiffDocument {
    let source = format!(
        "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n-old\n+{value}\n",
        path = path.display()
    );
    crate::git::diff::parse_diff(source.as_bytes(), title, Some(path), false)
}

fn loaded_two_file_changes_preview() -> (App, [PathBuf; 2], DiffIndex, DiffDocument) {
    let mut app = app_with_changes();
    for change in &mut app.status.changes {
        change.area = ChangeArea::Unstaged;
    }
    app.selected_change_section = Some(ChangeSection::Unstaged);
    let main_path = PathBuf::from("src/main.rs");
    let readme_path = PathBuf::from("README.md");
    let paths = [main_path.clone(), readme_path.clone()];
    let index = DiffIndex {
        title: "Current diff".to_owned(),
        files: paths
            .iter()
            .cloned()
            .map(|path| crate::git::diff::DiffFileIndexEntry {
                path,
                old_path: None,
                status: "modified".to_owned(),
                counts: None,
            })
            .collect(),
        truncated: false,
        commit_details: None,
    };
    let documents = HashMap::from([
        (
            main_path.clone(),
            parsed_change_document(&main_path, "Current diff", "current main"),
        ),
        (
            readme_path.clone(),
            parsed_change_document(&readme_path, "Current diff", "current readme"),
        ),
    ]);
    let document = index.document(&documents);
    app.document.clone_from(&document);
    app.changes_diff_version = 3;
    app.diff_generation = 5;
    app.local_diff_request = app.local_diff_request_for_view();
    app.local_diff_workspace_generation = Some(5);
    app.local_diff_index = Some(index.clone());
    app.local_diff_documents = documents;
    app.selected_preview_file = Some(readme_path.clone());
    app.preview_file_cursor = 1;
    app.files_collapsed = true;
    app.collapse_preference_set = true;
    app.expanded_preview_files = HashSet::from([main_path, readme_path]);
    (app, paths, index, document)
}

#[test]
fn refreshing_changes_keeps_the_loaded_diff_until_its_replacement_arrives() {
    let (mut app, path, index, loaded) = loaded_changes_preview();
    let now = Instant::now();

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), now);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::Refresh { .. })
    )));
    assert_eq!(app.document, loaded);

    let effects = app.handle_worker_event(
        WorkerEvent::Status {
            generation: app.status_generation,
            result: Ok(app.status.clone()),
        },
        now,
    );
    let generation = app.diff_generation;
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::PrepareLocalDiff {
                generation: command_generation,
                ..
            } if *command_generation == generation)
    )));
    assert_eq!(app.document, loaded);
    assert!(app.document_loading);

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation,
            result: Ok(index),
        },
        now,
    );
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadLocalDiffFile {
                generation: command_generation,
                workspace_generation,
                path: command_path,
            } if *command_generation == generation
                && *workspace_generation == generation
                && command_path == &path)
    )));
    assert_eq!(app.document, loaded);
    assert!(app.document_loading);

    let replacement = crate::git::diff::parse_diff(
        b"diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+replacement\n",
        "Replacement diff",
        Some(&path),
        false,
    );
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path,
            result: Ok(replacement.clone()),
        },
        now,
    ));

    assert_eq!(app.document, replacement);
    assert!(!app.document_loading);
}

#[test]
fn an_empty_refreshed_index_replaces_the_preserved_diff() {
    let (mut app, _, _, loaded) = loaded_changes_preview();
    let now = Instant::now();

    drop(app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), now));
    drop(app.handle_worker_event(
        WorkerEvent::Status {
            generation: app.status_generation,
            result: Ok(app.status.clone()),
        },
        now,
    ));
    let generation = app.diff_generation;
    assert_eq!(app.document, loaded);
    assert!(app.document_loading);

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation,
            result: Ok(DiffIndex {
                title: "Current diff".to_owned(),
                files: Vec::new(),
                truncated: false,
                commit_details: None,
            }),
        },
        now,
    );

    assert!(effects.is_empty());
    assert_eq!(
        app.document,
        DiffDocument::empty("Current diff", "No changes match the current filter")
    );
    assert!(!app.document_loading);
}

#[test]
fn a_two_file_refresh_replaces_the_preserved_diff_atomically() {
    let (mut app, [main_path, readme_path], index, loaded) = loaded_two_file_changes_preview();
    let now = Instant::now();
    app.content_scroll = 17;
    app.horizontal_scroll = 4;
    let expanded = app.expanded_preview_files.clone();

    drop(app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), now));
    drop(app.handle_worker_event(
        WorkerEvent::Status {
            generation: app.status_generation,
            result: Ok(app.status.clone()),
        },
        now,
    ));
    let generation = app.diff_generation;
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation,
            result: Ok(index.clone()),
        },
        now,
    );

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadLocalDiffFile {
                path,
                ..
            } if path == &main_path)
    )));
    assert_eq!(app.document, loaded);
    assert_eq!(app.selected_preview_file, Some(readme_path.clone()));
    assert_eq!(app.preview_file_cursor, 1);
    assert_eq!(app.expanded_preview_files, expanded);
    assert!(app.files_collapsed);
    assert_eq!(app.content_scroll, 17);
    assert_eq!(app.horizontal_scroll, 4);

    let replacement_main = parsed_change_document(&main_path, "Current diff", "replacement main");
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: main_path.clone(),
            result: Ok(replacement_main.clone()),
        },
        now,
    );

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadLocalDiffFile {
                path,
                ..
            } if path == &readme_path)
    )));
    assert_eq!(app.document, loaded);
    assert!(app.document_loading);

    let replacement_readme =
        parsed_change_document(&readme_path, "Current diff", "replacement readme");
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: readme_path.clone(),
            result: Ok(replacement_readme.clone()),
        },
        now,
    ));
    let replacement = index.document(&HashMap::from([
        (main_path, replacement_main),
        (readme_path.clone(), replacement_readme),
    ]));

    assert_eq!(app.document, replacement);
    assert!(!app.document_loading);
    assert_eq!(app.selected_preview_file, Some(readme_path));
    assert_eq!(app.preview_file_cursor, 1);
    assert_eq!(app.expanded_preview_files, expanded);
    assert!(app.files_collapsed);
    assert_eq!(app.content_scroll, 17);
    assert_eq!(app.horizontal_scroll, 4);
}

#[test]
fn coalesced_refresh_waits_for_the_latest_status_before_reloading_the_diff() {
    let (mut app, _, _, loaded) = loaded_changes_preview();
    let mut initial_effects = Vec::new();
    app.filesystem_changed(&mut initial_effects);
    app.filesystem_changed(&mut initial_effects);
    let now = Instant::now();

    assert_eq!(
        initial_effects
            .iter()
            .filter(|effect| matches!(
                effect,
                AppEffect::Git(command)
                    if matches!(command.as_ref(), WorkerCommand::Refresh { .. })
            ))
            .count(),
        1
    );
    assert!(app.refresh_again);

    let effects = app.handle_worker_event(
        WorkerEvent::Status {
            generation: app.status_generation,
            result: Ok(app.status.clone()),
        },
        now,
    );

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::Refresh { .. })
    )));
    assert!(effects.iter().all(|effect| !matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::PrepareLocalDiff { .. })
    )));
    assert_eq!(app.document, loaded);
    assert!(app.refreshing);

    let mut during_follow_up = Vec::new();
    app.filesystem_changed(&mut during_follow_up);
    assert!(during_follow_up.is_empty());
    assert!(app.refresh_again);

    let effects = app.handle_worker_event(
        WorkerEvent::Status {
            generation: app.status_generation,
            result: Err("refresh interrupted".to_owned()),
        },
        now,
    );

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::Refresh { .. })
    )));
    assert!(effects.iter().all(|effect| !matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::PrepareLocalDiff { .. })
    )));
    assert_eq!(app.document, loaded);
    assert!(app.refreshing);

    let effects = app.handle_worker_event(
        WorkerEvent::Status {
            generation: app.status_generation,
            result: Ok(app.status.clone()),
        },
        now,
    );

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::PrepareLocalDiff { .. })
    )));
    assert!(effects.iter().all(|effect| !matches!(
        effect,
        AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::Refresh { .. })
    )));
    assert_eq!(app.document, loaded);
    assert!(!app.refreshing);
}
