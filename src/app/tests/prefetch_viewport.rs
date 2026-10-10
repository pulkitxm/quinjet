use super::*;

fn large_pull_request() -> App {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_workspace_generation = Some(10);
    app.pull_request_files = (0..4_262)
        .map(|index| PullRequestFile {
            path: PathBuf::from(format!("file-{index:04}.txt")),
            old_path: None,
            status: PullRequestFileStatus::Added,
            counts: None,
        })
        .collect();
    app
}

#[test]
fn visible_files_reload_after_eviction_and_beyond_the_old_prefetch_limit() {
    let mut app = large_pull_request();
    for file in app.pull_request_files.clone() {
        app.cache_pull_request_document(file.path, DiffDocument::empty("patch", "loaded"));
    }
    app.prune_pull_request_documents(0);
    assert_eq!(app.pull_request_documents.len(), 1);
    let paths = vec![
        PathBuf::from("file-0000.txt"),
        PathBuf::from("file-4200.txt"),
    ];
    app.pull_request_viewport_paths.clone_from(&paths);

    let (effects, _) = app.tick(Instant::now());

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestFileBatch { workspace_generation: 10, paths: batch }
                if batch == &paths
        )
    ));
}

#[test]
fn scrolling_during_a_batch_prioritizes_the_new_viewport() {
    let mut app = large_pull_request();
    app.pull_request_viewport_paths = vec![PathBuf::from("file-0000.txt")];
    let (first, _) = app.tick(Instant::now());
    assert_eq!(first.len(), 1);
    app.pull_request_viewport_paths = vec![PathBuf::from("file-4261.txt")];
    let (in_flight, _) = app.tick(Instant::now());
    assert!(in_flight.is_empty());

    let next = app.handle_worker_event(
        WorkerEvent::PullRequestDiffBatch {
            workspace_generation: 10,
            result: Ok(vec![(
                PathBuf::from("file-0000.txt"),
                DiffDocument::empty("first", "loaded"),
            )]),
        },
        Instant::now(),
    );

    assert!(matches!(
        next.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestFileBatch { paths, .. }
                if paths == &[PathBuf::from("file-4261.txt")]
        )
    ));
}

#[test]
fn collapsed_and_background_views_do_not_load_patches() {
    let mut app = large_pull_request();
    app.pull_request_viewport_paths = vec![PathBuf::from("file-0000.txt")];
    app.set_document(indexed_document(&["file-0000.txt", "file-4261.txt"]));
    app.files_collapsed = true;
    assert!(app.tick(Instant::now()).0.is_empty());
    app.files_collapsed = false;
    app.tab_active = false;
    assert!(app.tick(Instant::now()).0.is_empty());
    app.tab_active = true;
    app.pull_request_section = PullRequestSection::Overview;
    assert!(app.tick(Instant::now()).0.is_empty());
}

#[test]
fn a_batch_with_no_documents_stops_retrying_and_shows_an_error() {
    let mut app = large_pull_request();
    let path = PathBuf::from("file-0000.txt");
    app.pull_request_viewport_paths = vec![path.clone()];
    assert_eq!(app.tick(Instant::now()).0.len(), 1);
    for _ in 0..2 {
        app.handle_worker_event(
            WorkerEvent::PullRequestDiffBatch {
                workspace_generation: 10,
                result: Ok(Vec::new()),
            },
            Instant::now(),
        );
    }
    assert!(app.tick(Instant::now()).0.is_empty());
    assert!(
        app.pull_request_documents[&path]
            .lines
            .iter()
            .any(|line| { line.text() == "Unable to load diff: No file diffs were returned" })
    );
}
