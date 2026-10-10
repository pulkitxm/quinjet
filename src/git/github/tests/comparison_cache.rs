use super::*;

#[test]
fn cached_patches_open_without_pr_objects_and_only_missing_files_fetch() {
    let source = initialized_repository();
    let remote = test_directory("cached-remote.git");
    source.git(&["init", "--bare", remote.0.to_str().unwrap()]);
    source.git(&["remote", "add", "test-origin", remote.0.to_str().unwrap()]);
    source.git(&["push", "test-origin", "main:refs/heads/main"]);
    let base_oid = source.git(&["rev-parse", "HEAD"]);
    source.git(&["switch", "-c", "feature/rocket"]);
    fs::write(source.0.join("cached.rs"), "pub const CACHED: usize = 1;\n").unwrap();
    fs::write(
        source.0.join("uncached.rs"),
        "pub const UNCACHED: usize = 2;\n",
    )
    .unwrap();
    source.git(&["add", "."]);
    source.git(&["commit", "--message=cache fixtures"]);
    source.git(&["push", "test-origin", "feature/rocket:refs/pull/7/head"]);
    let head_oid = source.git(&["rev-parse", "HEAD"]);
    let request = PullRequest {
        base_oid,
        head_oid,
        changed_files: 2,
        ..pull_request(
            repository("acme/widget", remote.0.to_str().unwrap(), &["origin"]),
            7,
        )
    };
    let local = Repository {
        root: source.0.clone(),
        github_cli: None,
        diff_highlighting: true,
    };
    let warm = local.prepare_pull_request_diff(&request, |_| {}).unwrap();
    let expected = warm.diff_file(Path::new("cached.rs")).unwrap();
    source.git(&["switch", "main"]);
    let from_main = local.prepare_pull_request_diff(&request, |_| {}).unwrap();
    assert_eq!(from_main.index(), warm.index());
    assert_eq!(
        from_main.diff_file(Path::new("cached.rs")).unwrap(),
        expected
    );

    let consumer = test_directory("consumer");
    consumer.git(&[
        "clone",
        "--no-local",
        "--branch=main",
        "--single-branch",
        remote.0.to_str().unwrap(),
        ".",
    ]);
    let opened = Repository {
        root: consumer.0.clone(),
        github_cli: None,
        diff_highlighting: true,
    };
    assert!(!opened.has_commit(&request.head_oid));
    let before_refs = consumer.git(&["show-ref"]);
    let before_status = consumer.git(&["status", "--porcelain"]);
    let unavailable = remote.0.with_extension("unavailable");
    fs::rename(&remote.0, &unavailable).unwrap();

    let workspace = opened.prepare_pull_request_diff(&request, |_| {}).unwrap();
    assert_eq!(workspace.index(), warm.index());
    assert_eq!(
        workspace.diff_file(Path::new("cached.rs")).unwrap(),
        expected
    );
    assert_eq!(
        workspace.diff_files(&[PathBuf::from("cached.rs")]).unwrap()[0].1,
        expected
    );
    drop(workspace.diff_file(Path::new("uncached.rs")).unwrap_err());

    fs::rename(&unavailable, &remote.0).unwrap();
    let documents = workspace
        .diff_files(&[PathBuf::from("cached.rs"), PathBuf::from("uncached.rs")])
        .unwrap();
    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].1, expected);
    assert!(
        documents[1]
            .1
            .lines
            .iter()
            .any(|line| line.text().contains("UNCACHED"))
    );
    assert!(!opened.has_commit(&request.head_oid));
    assert_eq!(consumer.git(&["branch", "--show-current"]), "main");
    assert_eq!(consumer.git(&["show-ref"]), before_refs);
    assert_eq!(consumer.git(&["status", "--porcelain"]), before_status);

    let changed = PullRequest {
        head_oid: "a".repeat(40),
        ..request.clone()
    };
    assert!(super::super::comparison_cache::load(&opened, &changed, false).is_none());
    assert!(super::super::comparison_cache::load(&opened, &request, true).is_none());
}
