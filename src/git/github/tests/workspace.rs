use super::*;

#[test]
fn changed_file_index_includes_add_modify_delete_and_rename_statuses() {
    let repository = initialized_repository();
    fs::write(repository.0.join("modified.txt"), "before\n").unwrap();
    fs::write(repository.0.join("deleted.txt"), "delete me\n").unwrap();
    fs::write(repository.0.join("renamed.txt"), "keep this content\n").unwrap();
    repository.git(&["add", "."]);
    repository.git(&["commit", "--message=fixtures"]);
    let base = repository.git(&["rev-parse", "HEAD"]);

    fs::write(repository.0.join("modified.txt"), "after\n").unwrap();
    fs::remove_file(repository.0.join("deleted.txt")).unwrap();
    fs::write(repository.0.join("added.txt"), "new\n").unwrap();
    repository.git(&["mv", "renamed.txt", "moved.txt"]);
    repository.git(&["add", "."]);
    repository.git(&["commit", "--message=changes"]);
    let head = repository.git(&["rev-parse", "HEAD"]);

    let (files, truncated) =
        changed_files_in_repository(&repository.0, &base, &head, None).unwrap();

    assert!(!truncated);
    assert!(files.iter().any(|file| {
        file.path == Path::new("added.txt") && file.status == PullRequestFileStatus::Added
    }));
    assert!(files.iter().any(|file| {
        file.path == Path::new("modified.txt") && file.status == PullRequestFileStatus::Modified
    }));
    assert!(files.iter().any(|file| {
        file.path == Path::new("deleted.txt") && file.status == PullRequestFileStatus::Deleted
    }));
    assert!(files.iter().any(|file| {
        file.path == Path::new("moved.txt")
            && file.old_path.as_deref() == Some(Path::new("renamed.txt"))
            && file.status == PullRequestFileStatus::Renamed
    }));

    assert!(files.iter().all(|file| file.counts.is_some()));
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == Path::new("modified.txt"))
            .and_then(|file| file.counts),
        Some(DiffLineCounts {
            additions: 1,
            deletions: 1,
            binary: false,
        })
    );
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == Path::new("moved.txt"))
            .and_then(|file| file.counts),
        Some(DiffLineCounts::default())
    );
}

#[test]
fn locally_available_pr_objects_avoid_disposable_fetches() {
    let source = initialized_repository();
    let base_oid = source.git(&["rev-parse", "HEAD"]);
    source.git(&["switch", "-c", "feature/local-preview"]);
    fs::write(source.0.join("local.txt"), "available locally\n").unwrap();
    source.git(&["add", "local.txt"]);
    source.git(&["commit", "--message=local preview"]);
    let head_oid = source.git(&["rev-parse", "HEAD"]);
    let git_repository = Repository {
        root: source.0.clone(),
    };
    let mut request = pull_request(
        repository(
            "acme/widget",
            "https://invalid.example.test/acme/widget",
            &["origin"],
        ),
        7,
    );
    request.base_oid = base_oid;
    request.head_oid = head_oid;
    request.changed_files = 1;

    let started = std::time::Instant::now();
    let workspace = git_repository
        .prepare_pull_request_diff(&request, |_| {})
        .unwrap();
    let index = workspace.index();
    let document = workspace.diff_file(&index.files[0].path).unwrap();
    let elapsed = started.elapsed();

    assert_eq!(index.files.len(), 1);
    assert_eq!(document.file_count(), 1);
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.text().contains("local.txt"))
    );
    assert!(elapsed < Duration::from_secs(2));
}

#[test]
fn disposable_pr_workspace_indexes_all_files_and_does_not_mutate_the_source() {
    let source = initialized_repository();
    let remote = test_directory("remote.git");
    source.git(&["init", "--bare", remote.0.to_str().unwrap()]);
    source.git(&["remote", "add", "test-origin", remote.0.to_str().unwrap()]);
    source.git(&["push", "test-origin", "main:refs/heads/main"]);
    source.git(&["switch", "-c", "feature/rocket"]);
    for index in 0..21 {
        fs::write(
            source.0.join(format!("rocket-{index:02}.txt")),
            format!("launch {index}\n"),
        )
        .unwrap();
    }
    source.git(&["add", "."]);
    source.git(&["commit", "--message=rocket"]);
    source.git(&["push", "test-origin", "feature/rocket:refs/pull/7/head"]);
    source.git(&["switch", "main"]);

    let before_branch = source.git(&["branch", "--show-current"]);
    let before_status = source.git(&["status", "--porcelain"]);
    let before_refs = source.git(&["show-ref"]);
    let git_repository = Repository {
        root: source.0.clone(),
    };
    let mut request = pull_request(
        repository("acme/widget", remote.0.to_str().unwrap(), &["test-origin"]),
        7,
    );
    request.base_oid.clear();
    request.head_oid.clear();
    request.head_repository = None;
    request.changed_files = 21;
    request.additions = 21;
    request.deletions = 0;

    let workspace = git_repository
        .prepare_pull_request_diff(&request, |_| {})
        .unwrap();
    let temporary_path = match &workspace.repository {
        PreparedRepository::Temporary(repository) => repository.path.clone(),
        PreparedRepository::Opened(_) => panic!("expected an isolated PR workspace"),
    };
    let index = workspace.index();
    assert_eq!(index.files.len(), 21);
    assert_eq!(index.total_files, 21);
    assert!(!index.truncated);
    assert!(temporary_path.exists());

    let mut additions = 0;
    let mut deletions = 0;
    for file in &index.files {
        let document = workspace.diff_file(&file.path).unwrap();
        assert_eq!(document.file_count(), 1);
        additions += document.addition_count();
        deletions += document.deletion_count();
    }
    assert_eq!((additions, deletions), (21, 0));

    let paths: Vec<PathBuf> = index.files.iter().map(|file| file.path.clone()).collect();
    let batch = workspace.diff_files(&paths).unwrap();
    assert_eq!(batch.len(), 21);
    assert_eq!(
        batch
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>(),
        paths
    );
    assert!(batch.iter().all(|(_, document)| document.file_count() == 1));
    assert_eq!(
        batch
            .iter()
            .map(|(_, document)| document.addition_count())
            .sum::<usize>(),
        21
    );
    assert_eq!(
        workspace
            .diff_files(&[PathBuf::from("never-changed.txt")])
            .unwrap(),
        Vec::new()
    );
    drop(workspace);
    assert!(!temporary_path.exists());
    assert_eq!(source.git(&["branch", "--show-current"]), before_branch);
    assert_eq!(source.git(&["status", "--porcelain"]), before_status);
    assert_eq!(source.git(&["show-ref"]), before_refs);
}

#[test]
fn temporary_bare_repository_is_removed_on_drop() {
    let path = {
        let repository = TemporaryBareRepository::new().unwrap();
        assert!(repository.path.exists());
        repository.path.clone()
    };
    assert!(!path.exists());
}

#[test]
fn bounded_runner_kills_oversized_git_output() {
    let repository = initialized_repository();
    fs::write(repository.0.join("large.txt"), "x".repeat(256 * 1024)).unwrap();
    repository.git(&["add", "large.txt"]);
    repository.git(&["commit", "--message=large"]);
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(&repository.0)
        .args(["show", "HEAD:large.txt"]);

    let output = run_bounded_command(&mut command, 1024, 1024).unwrap();

    assert!(output.stdout_truncated);
    assert_eq!(output.stdout.len(), 1024);
}

#[test]
fn rejects_malformed_pull_request_output_without_panicking() {
    let base = repository("acme/widget", "https://github.com/acme/widget", &["origin"]);
    parse_pull_requests(b"not tsv", &base, std::slice::from_ref(&base)).unwrap_err();
}

#[test]
fn matching_head_remotes_do_not_cross_enterprise_hosts() {
    let dot_com = repository("acme/widget", "https://github.com/acme/widget", &["public"]);
    let enterprise = repository(
        "acme/widget",
        "https://github.example.com/acme/widget",
        &["work"],
    );

    assert_eq!(
        matching_remotes(&[dot_com, enterprise], "github.example.com", "ACME/WIDGET"),
        vec!["work"]
    );
}

#[test]
fn merges_fetch_and_push_aliases_for_the_same_repository() {
    let mut repositories = BTreeMap::new();
    merge_repository(
        &mut repositories,
        repository("acme/widget", "https://github.com/acme/widget/", &[]),
        Some("origin"),
    );
    merge_repository(
        &mut repositories,
        repository("acme/widget", "https://github.com/ACME/WIDGET", &[]),
        Some("upstream"),
    );

    let repository = repositories.into_values().next().unwrap();
    assert_eq!(repository.url, "https://github.com/acme/widget");
    assert_eq!(repository.remotes, vec!["origin", "upstream"]);
}
