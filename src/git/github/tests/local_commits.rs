use std::fmt::Write as _;

use super::*;

fn local_request(source: &TestDirectory, base: String) -> (Repository, PullRequest) {
    let repository = Repository {
        root: source.0.clone(),
        github_cli: Some(source.0.join("unavailable-gh")),
        diff_highlighting: true,
    };
    let request = PullRequest {
        base_oid: base,
        head_oid: source.git(&["rev-parse", "HEAD"]),
        base_repository: super::repository("acme/widget", "https://github.com/acme/widget", &[]),
        number: 42,
        ..PullRequest::default()
    };
    (repository, request)
}

#[test]
fn local_commit_range_and_metadata_need_no_github_cli() {
    let source = initialized_repository();
    let base = source.git(&["rev-parse", "HEAD"]);
    source.git(&["switch", "-c", "feature"]);
    source.git(&["commit", "--allow-empty", "--message=first"]);
    let first = source.git(&["rev-parse", "HEAD"]);
    source.git(&["commit", "--allow-empty", "--message=second"]);
    let (repository, request) = local_request(&source, base);
    let before = source.git(&["show-ref"]);
    let result = repository.pull_request_commits(&request).unwrap();
    assert_eq!(result.total_commits, 2);
    assert_eq!(result.commits[0].oid, first);
    assert_eq!(result.commits[0].subject, "first");
    assert_eq!(result.commits[1].oid, request.head_oid);
    assert_eq!(result.commits[1].subject, "second");
    assert_eq!(result.commits[1].author, "Quinjet Test");
    assert!(result.commits[1].author_login.is_none());
    assert_eq!(
        result.commits[1].authored_at,
        source.git(&["show", "-s", "--format=%aI", "HEAD"])
    );
    assert_eq!(
        result.commits[1].url,
        format!("https://github.com/acme/widget/commit/{}", request.head_oid)
    );
    assert!(!result.truncated);
    assert_eq!(source.git(&["show-ref"]), before);
    assert_eq!(source.git(&["status", "--porcelain"]), "");
    assert_eq!(
        repository.pull_request_commits(&request).unwrap().commits,
        result.commits
    );
}

#[test]
fn local_commits_include_merges_and_exclude_the_advanced_base_history() {
    let source = initialized_repository();
    source.git(&["switch", "-c", "feature"]);
    source.git(&["commit", "--allow-empty", "--message=feature"]);
    let feature = source.git(&["rev-parse", "HEAD"]);
    source.git(&["switch", "main"]);
    source.git(&["commit", "--allow-empty", "--message=base advanced"]);
    let base = source.git(&["rev-parse", "HEAD"]);
    source.git(&["switch", "feature"]);
    source.git(&["merge", "--no-ff", "main", "--message=merge base"]);
    let (repository, request) = local_request(&source, base.clone());
    let result = repository.pull_request_commits(&request).unwrap();
    assert_eq!(result.total_commits, 2);
    assert_eq!(result.commits[0].oid, feature);
    assert_eq!(result.commits[1].oid, request.head_oid);
    assert!(!result.commits.iter().any(|commit| commit.oid == base));
}

#[test]
fn a_changed_head_reads_new_local_commits() {
    let source = initialized_repository();
    let base = source.git(&["rev-parse", "HEAD"]);
    source.git(&["commit", "--allow-empty", "--message=first"]);
    let (repository, mut request) = local_request(&source, base);
    assert_eq!(
        repository
            .pull_request_commits(&request)
            .unwrap()
            .total_commits,
        1
    );
    source.git(&["commit", "--allow-empty", "--message=new push"]);
    request.head_oid = source.git(&["rev-parse", "HEAD"]);
    let result = repository.pull_request_commits(&request).unwrap();
    assert_eq!(result.total_commits, 2);
    assert_eq!(result.commits[1].subject, "new push");
}

#[test]
fn shallow_history_cut_inside_the_range_is_not_returned_as_complete() {
    let source = initialized_repository();
    let base = source.git(&["rev-parse", "HEAD"]);
    source.git(&["commit", "--allow-empty", "--message=first"]);
    let cut = source.git(&["rev-parse", "HEAD"]);
    source.git(&["commit", "--allow-empty", "--message=second"]);
    let (repository, request) = local_request(&source, base);
    fs::write(source.0.join(".git/shallow"), format!("{cut}\n")).unwrap();
    drop(repository.pull_request_commits(&request).unwrap_err());
}

#[test]
fn shallow_history_outside_the_comparison_does_not_force_a_cloud_read() {
    let source = initialized_repository();
    let cut = source.git(&["rev-parse", "HEAD"]);
    source.git(&["commit", "--allow-empty", "--message=base"]);
    let base = source.git(&["rev-parse", "HEAD"]);
    source.git(&["commit", "--allow-empty", "--message=feature"]);
    let (repository, request) = local_request(&source, base);
    fs::write(source.0.join(".git/shallow"), format!("{cut}\n")).unwrap();
    assert_eq!(
        repository
            .pull_request_commits(&request)
            .unwrap()
            .total_commits,
        1
    );
}

#[test]
fn large_local_ranges_return_every_commit_without_github() {
    let source = initialized_repository();
    let base = source.git(&["rev-parse", "HEAD"]);
    let mut input = String::new();
    for index in 0..16_385 {
        let subject = format!("commit {index}");
        writeln!(input, "commit refs/heads/large\nmark :{}", index + 1).unwrap();
        writeln!(
            input,
            "committer Quinjet Test <quinjet@example.test> 1700000000 +0000"
        )
        .unwrap();
        writeln!(input, "data {}\n{subject}", subject.len()).unwrap();
        if index == 0 {
            writeln!(input, "from {base}\n").unwrap();
        } else {
            writeln!(input, "from :{index}\n").unwrap();
        }
    }
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(&source.0)
        .args(["fast-import", "--quiet"]);
    let output =
        process::run_bounded_command_with_input(&mut command, input.as_bytes(), 1024, 1024)
            .unwrap();
    assert!(output.status.success(), "{}", text(&output.stderr));
    let head = source.git(&["rev-parse", "refs/heads/large"]);
    source.git(&["update-ref", "refs/heads/main", &head]);
    let (repository, request) = local_request(&source, base);
    let result = repository.pull_request_commits(&request).unwrap();
    assert_eq!(result.total_commits, 16_385);
    assert_eq!(result.commits.len(), 16_385);
    assert_eq!(result.commits[0].subject, "commit 0");
    assert_eq!(result.commits[16_384].oid, head);
    assert!(!result.truncated);
}
