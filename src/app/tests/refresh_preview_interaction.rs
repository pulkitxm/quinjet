use super::refresh_preview::{
    loaded_changes_preview, loaded_two_file_changes_preview, parsed_change_document,
    refresh_with_status,
};
use super::*;

fn index_entry(path: PathBuf, status: &str) -> crate::git::diff::DiffFileIndexEntry {
    crate::git::diff::DiffFileIndexEntry {
        path,
        old_path: None,
        status: status.to_owned(),
        counts: None,
    }
}

#[test]
fn expanding_all_during_refresh_loads_new_files_before_the_atomic_swap() {
    let (mut app, [main_path, readme_path], mut index, loaded) = loaded_two_file_changes_preview();
    app.files_collapsed = false;
    app.collapse_preference_set = false;
    app.expanded_preview_files.clear();
    app.collapsed_preview_files = HashSet::from([main_path.clone(), readme_path.clone()]);
    let now = Instant::now();
    let new_path = PathBuf::from("src/new.rs");
    let mut status = app.status.clone();
    status.changes.push(Change {
        path: new_path.clone(),
        original_path: None,
        area: ChangeArea::Unstaged,
        status: ChangeStatus::Added,
    });
    drop(refresh_with_status(&mut app, status, now));
    index.files.push(index_entry(new_path.clone(), "added"));
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: app.diff_generation,
            result: Ok(index),
        },
        now,
    ));

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), now);

    assert!(effects.is_empty());
    assert_eq!(app.document, loaded);
    assert!(app.local_diff_pending_paths.contains(&main_path));
    assert!(app.local_diff_pending_paths.contains(&new_path));
    let generation = app.diff_generation;
    let replacement_readme =
        parsed_change_document(&readme_path, "Current diff", "replacement readme");
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: readme_path.clone(),
            result: Ok(replacement_readme),
        },
        now,
    );
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadLocalDiffFile { path, .. }
                if path == &main_path)
    )));
    let replacement_main = parsed_change_document(&main_path, "Current diff", "replacement main");
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: main_path.clone(),
            result: Ok(replacement_main),
        },
        now,
    );
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadLocalDiffFile { path, .. }
                if path == &new_path)
    )));
    let replacement_new = parsed_change_document(&new_path, "Current diff", "replacement new");
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: new_path.clone(),
            result: Ok(replacement_new),
        },
        now,
    ));

    assert!(!app.document_loading);
    assert_eq!(app.document.file_count(), 3);
    assert!(
        app.document
            .lines
            .iter()
            .all(|line| line.text() != "Loading diff…")
    );
    assert!(!app.preview_file_collapsed(&main_path.to_string_lossy()));
    assert!(!app.preview_file_collapsed(&readme_path.to_string_lossy()));
    assert!(!app.preview_file_collapsed(&new_path.to_string_lossy()));
}

#[test]
fn space_toggles_the_rendered_file_before_the_refreshed_index_arrives() {
    let (mut app, [_, readme_path], _, loaded) = loaded_two_file_changes_preview();
    app.focus = Focus::Content;
    let now = Instant::now();
    let status = app.status.clone();
    drop(refresh_with_status(&mut app, status, now));

    assert!(app.local_diff_index.is_none());
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), now);

    assert!(effects.is_empty());
    assert_eq!(app.document, loaded);
    assert!(app.preview_file_collapsed(&readme_path.to_string_lossy()));
    assert!(!app.expanded_preview_files.contains(&readme_path));
}

#[test]
fn superseding_a_refresh_clears_its_queue_and_rejects_old_workspace_loads() {
    let (mut app, _, mut index, loaded) = loaded_two_file_changes_preview();
    app.files_collapsed = false;
    app.collapse_preference_set = true;
    app.collapsed_preview_files.clear();
    app.expanded_preview_files.clear();
    let now = Instant::now();
    let new_path = PathBuf::from("src/new.rs");
    let mut status = app.status.clone();
    status.changes.push(Change {
        path: new_path.clone(),
        original_path: None,
        area: ChangeArea::Unstaged,
        status: ChangeStatus::Added,
    });
    drop(refresh_with_status(&mut app, status, now));
    index.files.push(index_entry(new_path.clone(), "added"));
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: app.diff_generation,
            result: Ok(index),
        },
        now,
    ));
    assert!(app.local_diff_pending_paths.contains(&new_path));

    app.filesystem_changed(now);
    let (refresh_effects, _) = app.tick(now + FILESYSTEM_REFRESH_DEBOUNCE);
    assert!(refresh_effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::Refresh { .. })
    )));

    assert!(app.local_diff_loading_path.is_none());
    assert!(app.local_diff_pending_paths.is_empty());
    drop(app.handle_worker_event(
        WorkerEvent::Status {
            generation: app.status_generation,
            result: Err("refresh interrupted".to_owned()),
        },
        now,
    ));
    let mut load_effects = Vec::new();
    app.request_local_diff_file(new_path, &mut load_effects);
    assert!(load_effects.is_empty());
    assert_eq!(app.document, loaded);
    assert!(!app.local_diff_preserving_document);
    assert!(app.local_diff_index.is_none());
    assert!(app.local_diff_workspace_generation.is_none());
}

#[test]
fn a_preserved_index_failure_keeps_the_rendered_diff() {
    let (mut app, [main_path, readme_path], _, loaded) = loaded_two_file_changes_preview();
    let selected = app.selected_preview_file.clone();
    let expanded = app.expanded_preview_files.clone();
    let now = Instant::now();
    let status = app.status.clone();
    drop(refresh_with_status(&mut app, status, now));

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: app.diff_generation,
            result: Err("unable to refresh index".to_owned()),
        },
        now,
    );

    assert!(effects.is_empty());
    assert_eq!(app.document, loaded);
    assert_eq!(app.selected_preview_file, selected);
    assert_eq!(app.expanded_preview_files, expanded);
    assert!(!app.document_loading);
    assert!(!app.local_diff_preserving_document);
    assert_eq!(
        app.local_diff_preserved_paths,
        HashSet::from([main_path, readme_path])
    );
    assert!(app.local_diff_request.is_some());
    assert!(app.local_diff_index.is_none());
}

#[test]
fn a_preserved_file_failure_keeps_the_rendered_diff() {
    let (mut app, path, index, loaded) = loaded_changes_preview();
    let now = Instant::now();
    let status = app.status.clone();
    drop(refresh_with_status(&mut app, status, now));
    let generation = app.diff_generation;
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation,
            result: Ok(index),
        },
        now,
    ));

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: path.clone(),
            result: Err("unable to refresh file".to_owned()),
        },
        now,
    );

    assert!(effects.is_empty());
    assert_eq!(app.document, loaded);
    assert_eq!(app.selected_preview_file, Some(path.clone()));
    assert!(!app.document_loading);
    assert!(!app.local_diff_preserving_document);
    assert_eq!(app.local_diff_preserved_paths, HashSet::from([path]));
    assert!(app.local_diff_request.is_some());
    assert!(app.local_diff_index.is_none());
}

#[test]
fn expanding_a_lazy_file_after_a_failed_refresh_uses_the_retained_workspace() {
    let (mut app, [_, readme_path], ..) = loaded_two_file_changes_preview();
    app.focus = Focus::Content;
    app.files_collapsed = false;
    app.collapse_preference_set = true;
    app.collapsed_preview_files = HashSet::from([readme_path.clone()]);
    app.expanded_preview_files.clear();
    drop(app.local_diff_documents.remove(&readme_path));
    app.rebuild_local_diff_document();
    app.selected_preview_file = Some(readme_path.clone());
    app.preview_file_cursor = 1;
    let retained = app.document.clone();
    let workspace_generation = app.local_diff_workspace_generation;
    let now = Instant::now();
    drop(app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), now));
    let generation = app.diff_generation;
    drop(app.handle_worker_event(
        WorkerEvent::Status {
            generation: app.status_generation,
            result: Err("unable to refresh status".to_owned()),
        },
        now,
    ));

    assert_eq!(app.document, retained);
    assert_eq!(app.local_diff_workspace_generation, workspace_generation);
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile {
                generation: command_generation,
                workspace_generation: command_workspace,
                path,
            } if *command_generation == generation
                && Some(*command_workspace) == workspace_generation
                && path == &readme_path
        )
    ));
    assert_eq!(app.local_diff_loading_path, Some(readme_path.clone()));
    assert!(
        app.document
            .lines
            .iter()
            .any(|line| line.text() == "Loading diff…")
    );

    let replacement = parsed_change_document(&readme_path, "Current diff", "replacement readme");
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: workspace_generation.unwrap_or_default(),
            path: readme_path,
            result: Ok(replacement),
        },
        now,
    ));

    assert!(
        app.document
            .lines
            .iter()
            .any(|line| line.text().contains("replacement readme"))
    );
    assert!(
        app.document
            .lines
            .iter()
            .all(|line| line.text() != "Loading diff…")
    );
}

#[test]
fn a_background_status_failure_does_not_abort_a_preserved_refresh() {
    let (mut app, [main_path, readme_path], index, retained) = loaded_two_file_changes_preview();
    let selected = app.selected_preview_file.clone();
    let expanded = app.expanded_preview_files.clone();
    let now = Instant::now();
    let status = app.status.clone();
    drop(refresh_with_status(&mut app, status, now));
    let generation = app.diff_generation;
    let mut effects = Vec::new();
    app.periodic_refresh(&mut effects);
    drop(app.handle_worker_event(
        WorkerEvent::Status {
            generation: app.status_generation,
            result: Err("background refresh failed".to_owned()),
        },
        now,
    ));

    assert!(app.local_diff_preserving_document);
    assert!(app.document_loading);
    assert_eq!(app.document, retained);
    assert_eq!(app.selected_preview_file, selected);
    assert_eq!(app.expanded_preview_files, expanded);
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation,
            result: Ok(index),
        },
        now,
    );

    assert_eq!(app.document, retained);
    assert_eq!(app.selected_preview_file, selected);
    assert_eq!(app.expanded_preview_files, expanded);
    assert!(app.local_diff_preserving_document);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadLocalDiffFile { path, .. }
                if path == &main_path)
    )));
    assert!(app.local_diff_pending_paths.contains(&readme_path));

    let replacement_main = parsed_change_document(&main_path, "Current diff", "replacement main");
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: main_path,
            result: Ok(replacement_main),
        },
        now,
    );
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadLocalDiffFile { path, .. }
                if path == &readme_path)
    )));
    let replacement_readme =
        parsed_change_document(&readme_path, "Current diff", "replacement readme");
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: readme_path,
            result: Ok(replacement_readme),
        },
        now,
    ));

    assert!(!app.local_diff_preserving_document);
    assert!(!app.document_loading);
    assert_eq!(app.selected_preview_file, selected);
    assert_eq!(app.expanded_preview_files, expanded);
    assert!(
        app.document
            .lines
            .iter()
            .any(|line| line.text().contains("replacement readme"))
    );
}
