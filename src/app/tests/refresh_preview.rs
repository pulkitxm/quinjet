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
