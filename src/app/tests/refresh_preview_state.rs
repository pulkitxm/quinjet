use super::refresh_preview::{
    loaded_changes_preview, loaded_two_file_changes_preview, parsed_change_document,
    refresh_with_status,
};
use super::*;
use crate::git::diff::DiffLine;

fn index_entry(path: PathBuf, status: &str) -> crate::git::diff::DiffFileIndexEntry {
    crate::git::diff::DiffFileIndexEntry {
        path,
        old_path: None,
        status: status.to_owned(),
        counts: None,
    }
}

fn assert_prepare_local_diff(effects: &[AppEffect]) {
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::PrepareLocalDiff { .. })
    )));
}

fn assert_indexing_placeholder(app: &App) {
    assert_eq!(app.document.file_count(), 0);
    assert_eq!(
        app.document.lines.first().map(DiffLine::text).as_deref(),
        Some("Indexing changed files…")
    );
}

#[test]
fn a_selected_section_preserves_membership_and_applies_folds_at_the_swap() {
    let (mut app, [main_path, readme_path], mut index, loaded) = loaded_two_file_changes_preview();
    let now = Instant::now();
    app.files_collapsed = false;
    app.collapse_preference_set = false;
    app.expanded_preview_files.clear();
    app.collapsed_preview_files = HashSet::from([readme_path.clone()]);
    let mut status = app.status.clone();
    let new_path = PathBuf::from("src/new.rs");
    status.changes.push(Change {
        path: new_path.clone(),
        original_path: None,
        area: ChangeArea::Unstaged,
        status: ChangeStatus::Added,
    });

    let effects = refresh_with_status(&mut app, status, now);

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::PrepareLocalDiff {
                request,
                ..
            } if matches!(request.as_ref(), LocalDiffRequest::Changes { changes, .. }
                if changes.len() == 3))
    )));
    assert_eq!(app.document, loaded);
    assert_eq!(app.local_diff_change_section, Some(ChangeSection::Unstaged));
    index.files.push(index_entry(new_path.clone(), "added"));

    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: app.diff_generation,
            result: Ok(index),
        },
        now,
    ));

    assert_eq!(app.document, loaded);
    assert_eq!(
        app.collapsed_preview_files,
        HashSet::from([readme_path.clone()])
    );
    assert!(!app.local_diff_pending_paths.contains(&new_path));
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
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: main_path.clone(),
            result: Ok(replacement_main),
        },
        now,
    ));

    assert!(!app.document_loading);
    assert!(!app.collapsed_preview_files.contains(&main_path));
    assert!(app.collapsed_preview_files.contains(&readme_path));
    assert!(app.collapsed_preview_files.contains(&new_path));
    assert!(app.local_diff_preserved_paths.is_empty());
}

#[test]
fn an_individual_change_preserves_its_diff_when_its_status_changes() {
    let (mut app, _, _, loaded) = loaded_changes_preview();
    let now = Instant::now();
    let mut status = app.status.clone();
    for change in &mut status.changes {
        if change.path == Path::new("src/main.rs") {
            change.status = ChangeStatus::TypeChanged;
        }
    }

    let effects = refresh_with_status(&mut app, status, now);

    assert_prepare_local_diff(&effects);
    assert_eq!(app.document, loaded);
    assert!(app.document_loading);
}

#[test]
fn selecting_a_different_change_replaces_the_previous_diff_with_indexing() {
    let (mut app, _, _, _) = loaded_changes_preview();
    let now = Instant::now();
    let mut status = app.status.clone();
    status
        .changes
        .retain(|change| change.path == Path::new("README.md"));

    let effects = refresh_with_status(&mut app, status, now);

    assert_prepare_local_diff(&effects);
    assert_indexing_placeholder(&app);
    assert!(app.document_loading);
    assert!(app.selected_preview_file.is_none());
    assert!(app.collapsed_preview_files.is_empty());
    assert!(app.expanded_preview_files.is_empty());
}

#[test]
fn moving_the_same_path_to_another_area_does_not_preserve_its_diff() {
    let (mut app, _, _, _) = loaded_changes_preview();
    let now = Instant::now();
    let mut status = app.status.clone();
    status
        .changes
        .retain(|change| change.path == Path::new("src/main.rs"));
    for change in &mut status.changes {
        change.area = ChangeArea::Staged;
    }

    let effects = refresh_with_status(&mut app, status, now);

    assert_prepare_local_diff(&effects);
    assert_indexing_placeholder(&app);
}

#[test]
fn a_superseded_index_cannot_change_the_rendered_selection_or_folds() {
    let (mut app, [main_path, readme_path], index, loaded) = loaded_two_file_changes_preview();
    let now = Instant::now();
    let status = app.status.clone();
    drop(refresh_with_status(&mut app, status, now));
    let intermediate_generation = app.diff_generation;
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: intermediate_generation,
            result: Ok(DiffIndex {
                title: "Intermediate diff".to_owned(),
                files: vec![index_entry(main_path.clone(), "modified")],
                truncated: false,
                commit_details: None,
            }),
        },
        now,
    ));

    assert_eq!(app.document, loaded);
    assert_eq!(app.selected_preview_file, Some(readme_path.clone()));
    assert!(app.expanded_preview_files.contains(&readme_path));
    assert!(!app.preview_file_collapsed(&readme_path.to_string_lossy()));
    let mut refresh_effects = Vec::new();
    app.filesystem_changed(&mut refresh_effects);
    assert!(refresh_effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::Refresh { .. })
    )));
    let effects = app.handle_worker_event(
        WorkerEvent::Status {
            generation: app.status_generation,
            result: Ok(app.status.clone()),
        },
        now,
    );
    assert_prepare_local_diff(&effects);
    let final_generation = app.diff_generation;
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: final_generation,
            result: Ok(index.clone()),
        },
        now,
    ));

    assert_eq!(app.document, loaded);
    assert_eq!(app.selected_preview_file, Some(readme_path.clone()));
    assert!(app.expanded_preview_files.contains(&readme_path));
    assert_eq!(
        app.local_diff_preserved_paths,
        HashSet::from([main_path.clone(), readme_path.clone()])
    );
    let replacement_main = parsed_change_document(&main_path, "Current diff", "replacement main");
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: final_generation,
            workspace_generation: final_generation,
            path: main_path.clone(),
            result: Ok(replacement_main.clone()),
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
            generation: final_generation,
            workspace_generation: final_generation,
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
    assert_eq!(app.selected_preview_file, Some(readme_path.clone()));
    assert!(app.expanded_preview_files.contains(&readme_path));
    assert!(!app.preview_file_collapsed(&readme_path.to_string_lossy()));
}

#[test]
fn a_completed_refresh_prunes_removed_file_state() {
    let (mut app, [main_path, readme_path], mut index, loaded) = loaded_two_file_changes_preview();
    let now = Instant::now();
    app.collapsed_preview_files.insert(readme_path.clone());
    let mut status = app.status.clone();
    status.changes.retain(|change| change.path == main_path);
    drop(refresh_with_status(&mut app, status, now));
    index.files.retain(|file| file.path == main_path);
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: app.diff_generation,
            result: Ok(index),
        },
        now,
    ));

    assert_eq!(app.document, loaded);
    assert_eq!(app.selected_preview_file, Some(readme_path.clone()));
    assert!(app.collapsed_preview_files.contains(&readme_path));
    let generation = app.diff_generation;
    let replacement = parsed_change_document(&main_path, "Current diff", "replacement main");
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: main_path.clone(),
            result: Ok(replacement.clone()),
        },
        now,
    ));

    assert_eq!(app.document, replacement);
    assert_eq!(app.selected_preview_file, Some(main_path.clone()));
    assert!(app.expanded_preview_files.contains(&main_path));
    assert!(!app.expanded_preview_files.contains(&readme_path));
    assert!(!app.collapsed_preview_files.contains(&readme_path));
}

#[test]
fn growing_a_single_file_section_auto_collapses_every_file() {
    let (mut app, path, mut index, loaded) = loaded_changes_preview();
    app.reset_local_diff_runtime();
    app.selected_change_section = Some(ChangeSection::Unstaged);
    app.changes_diff_version = 3;
    if let Some(request) = app.local_diff_request_for_view() {
        app.prepare_local_diff(request, &mut Vec::new());
    }
    app.document = loaded;
    app.document_loading = false;
    app.diff_generation = 5;
    app.local_diff_workspace_generation = Some(5);
    app.local_diff_index = Some(index.clone());
    app.local_diff_single_loaded = true;
    app.selected_preview_file = Some(path.clone());
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

    assert!(!app.local_diff_pending_paths.contains(&new_path));
    let generation = app.diff_generation;
    let replacement = parsed_change_document(&path, "Current diff", "replacement main");
    drop(app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation,
            workspace_generation: generation,
            path: path.clone(),
            result: Ok(replacement),
        },
        now,
    ));

    assert!(!app.document_loading);
    assert_eq!(app.collapsed_preview_files, HashSet::from([path, new_path]));
}
