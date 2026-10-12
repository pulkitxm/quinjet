use super::*;

#[test]
fn a_selected_commit_diff_excludes_earlier_and_later_changes() {
    let source = initialized_repository();
    let base = source.git(&["rev-parse", "HEAD"]);
    source.git(&["switch", "-c", "feature/rocket"]);
    fs::write(source.0.join("earlier.txt"), "earlier\n").unwrap();
    source.git(&["add", "."]);
    source.git(&["commit", "--message=earlier"]);
    fs::write(source.0.join("selected.txt"), "selected\n").unwrap();
    source.git(&["add", "."]);
    source.git(&["commit", "--message=selected"]);
    let selected = source.git(&["rev-parse", "HEAD"]);
    fs::write(source.0.join("later.txt"), "later\n").unwrap();
    source.git(&["add", "."]);
    source.git(&["commit", "--message=later"]);
    let head = source.git(&["rev-parse", "HEAD"]);
    let before = source.git(&["show-ref"]);
    let repository = Repository {
        root: source.0.clone(),
        github_cli: None,
        diff_highlighting: true,
    };
    let request = PullRequest {
        base_oid: base,
        head_oid: head,
        ..PullRequest::default()
    };
    let prepared = repository
        .prepare_pull_request_commit_diff(&request, &selected, |_| {})
        .unwrap();
    assert_eq!(prepared.index.files.len(), 1);
    assert_eq!(prepared.index.files[0].path, Path::new("selected.txt"));
    assert_eq!(prepared.index.files[0].counts.unwrap().additions, 1);
    let document = prepared.diff_file(Path::new("selected.txt")).unwrap();
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.text().contains("selected"))
    );
    assert_eq!(source.git(&["show-ref"]), before);
    assert_eq!(source.git(&["status", "--porcelain"]), "");
}

#[test]
fn merge_commit_diff_uses_its_first_parent() {
    let source = initialized_repository();
    let base = source.git(&["rev-parse", "HEAD"]);
    source.git(&["switch", "-c", "side"]);
    fs::write(source.0.join("side.txt"), "side\n").unwrap();
    source.git(&["add", "."]);
    source.git(&["commit", "--message=side"]);
    source.git(&["switch", "main"]);
    fs::write(source.0.join("main.txt"), "main\n").unwrap();
    source.git(&["add", "."]);
    source.git(&["commit", "--message=main"]);
    let parent = source.git(&["rev-parse", "HEAD"]);
    source.git(&["merge", "--no-ff", "side", "--message=merge"]);
    let selected = source.git(&["rev-parse", "HEAD"]);
    let repository = Repository {
        root: source.0.clone(),
        github_cli: None,
        diff_highlighting: true,
    };
    let request = PullRequest {
        base_oid: base,
        head_oid: selected.clone(),
        ..PullRequest::default()
    };
    let prepared = repository
        .prepare_pull_request_commit_diff(&request, &selected, |_| {})
        .unwrap();
    assert_eq!(prepared.merge_base, parent);
    assert_eq!(prepared.index.files.len(), 1);
    assert_eq!(prepared.index.files[0].path, Path::new("side.txt"));
}

#[cfg(unix)]
#[test]
fn remote_commit_older_than_the_initial_fetch_window_is_fetched_exactly() {
    use std::os::unix::fs::PermissionsExt;

    let source = initialized_repository();
    let base = source.git(&["rev-parse", "HEAD"]);
    let remote = test_directory("commit-remote.git");
    source.git(&[
        "init",
        "--bare",
        "--initial-branch=main",
        remote.0.to_str().unwrap(),
    ]);
    source.git(&["remote", "add", "test-origin", remote.0.to_str().unwrap()]);
    source.git(&["push", "test-origin", "main:main"]);
    source.git(&["switch", "-c", "feature/rocket"]);
    fs::write(source.0.join("selected.txt"), "selected\n").unwrap();
    source.git(&["add", "."]);
    source.git(&["commit", "--message=selected"]);
    let selected = source.git(&["rev-parse", "HEAD"]);
    for index in 0..70 {
        fs::write(source.0.join("later.txt"), format!("later {index}\n")).unwrap();
        source.git(&["add", "."]);
        source.git(&["commit", "--message=later"]);
    }
    let head = source.git(&["rev-parse", "HEAD"]);
    source.git(&["push", "test-origin", "HEAD:refs/pull/7/head"]);
    let opened = test_directory("commit-opened");
    source.git(&[
        "clone",
        "--depth=1",
        &format!("file://{}", remote.0.display()),
        opened.0.to_str().unwrap(),
    ]);
    let fake_gh = opened.0.join("fake-gh");
    fs::write(&fake_gh, format!("#!/bin/sh\nprintf '%s\\n' '{base}'\n")).unwrap();
    fs::set_permissions(&fake_gh, fs::Permissions::from_mode(0o755)).unwrap();
    let repository = Repository {
        root: opened.0.clone(),
        github_cli: Some(fake_gh),
        diff_highlighting: true,
    };
    assert!(!repository.has_commit(&selected));
    let before = opened.git(&["show-ref"]);
    let mut request = pull_request(
        super::repository(
            "acme/widget",
            &format!("file://{}", remote.0.display()),
            &[],
        ),
        7,
    );
    request.base_oid = base;
    request.head_oid = head.clone();
    let prepared = repository
        .prepare_pull_request_commit_diff(&request, &selected, |_| {})
        .unwrap();
    assert_eq!(prepared.head, selected);
    assert_eq!(prepared.index.files.len(), 1);
    assert_eq!(prepared.index.files[0].path, Path::new("selected.txt"));
    assert_eq!(opened.git(&["show-ref"]), before);

    source.git(&["push", "test-origin", "HEAD:refs/heads/feature/rocket"]);
    let shallow = test_directory("commit-shallow-head");
    source.git(&[
        "clone",
        "--depth=1",
        "--branch=feature/rocket",
        &format!("file://{}", remote.0.display()),
        shallow.0.to_str().unwrap(),
    ]);
    let repository = Repository {
        root: shallow.0.clone(),
        github_cli: None,
        diff_highlighting: true,
    };
    assert!(repository.has_commit(&head));
    let prepared = repository
        .prepare_pull_request_commit_diff(&request, &head, |_| {})
        .unwrap();
    assert_eq!(prepared.head, head);
    assert_eq!(prepared.index.files.len(), 1);
    assert_eq!(prepared.index.files[0].path, Path::new("later.txt"));
}
