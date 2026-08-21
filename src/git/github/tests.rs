
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

static TEST_REPOSITORY_ID: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

fn test_directory(label: &str) -> TestDirectory {
    let id = TEST_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
    // nosemgrep: rust.lang.security.temp-dir.temp-dir
    let path = env::temp_dir().join(format!(
        "quinjet-github-{label}-{}-{id}",
        std::process::id()
    ));
    drop(fs::remove_dir_all(&path));
    fs::create_dir_all(&path).unwrap();
    TestDirectory(path)
}

fn initialized_repository() -> TestDirectory {
    let directory = test_directory("repo");
    directory.git(&["init", "--initial-branch=main"]);
    directory.git(&["config", "user.name", "Quinjet Test"]);
    directory.git(&["config", "user.email", "quinjet@example.com"]);
    fs::write(directory.0.join("README.md"), "base\n").unwrap();
    directory.git(&["add", "README.md"]);
    directory.git(&["commit", "--message=base"]);
    directory
}

pub(super) fn repository(name: &str, url: &str, remotes: &[&str]) -> GitHubRepository {
    GitHubRepository {
        name_with_owner: name.to_owned(),
        url: url.to_owned(),
        remotes: remotes.iter().map(|remote| (*remote).to_owned()).collect(),
    }
}

pub(super) fn pull_request(base: GitHubRepository, number: u64) -> PullRequest {
    PullRequest {
        number,
        title: "Ship the rocket".to_owned(),
        description: "Launch safely".to_owned(),
        author: "octocat".to_owned(),
        state: "OPEN".to_owned(),
        is_draft: false,
        created_at: "2026-08-12T09:00:00Z".to_owned(),
        updated_at: "2026-08-13T12:00:00Z".to_owned(),
        url: format!("{}/pull/{number}", base.url),
        base_ref: "main".to_owned(),
        base_oid: String::new(),
        head_ref: "feature/rocket".to_owned(),
        head_oid: String::new(),
        base_repository: base,
        head_repository: Some("octocat/widget".to_owned()),
        head_remotes: vec!["origin".to_owned()],
        is_cross_repository: true,
        additions: 1,
        deletions: 0,
        changed_files: 1,
    }
}

#[test]
fn pull_request_operation_messages_name_the_number_and_title() {
    let pull_request = pull_request(
        repository(
            "pulkitxm/quinjet",
            "https://github.com/pulkitxm/quinjet",
            &[],
        ),
        12,
    );
    let merge = PullRequestOperation::Merge {
        method: PullRequestMergeMethod::Squash,
        delete_branch: false,
    };
    assert_eq!(
        merge.confirm_message(&pull_request),
        "Really squash and merge #12 (Ship the rocket)?"
    );
    assert_eq!(
        merge.success_message(&pull_request),
        "Squashed and merged #12"
    );
    assert_eq!(
        PullRequestOperation::Close.confirm_message(&pull_request),
        "Really close #12 (Ship the rocket)?"
    );
    assert_eq!(
        PullRequestOperation::Reopen.success_message(&pull_request),
        "Reopened #12"
    );
}

#[test]
fn discovers_distinct_fetch_and_push_repositories_for_each_remote() {
    let directory = initialized_repository();
    directory.git(&[
        "remote",
        "add",
        "origin",
        "https://github.com/acme/widget.git",
    ]);
    directory.git(&[
        "remote",
        "set-url",
        "--push",
        "origin",
        "git@github.com:octocat/widget.git",
    ]);
    directory.git(&[
        "remote",
        "add",
        "upstream",
        "https://github.com/acme/widget.git",
    ]);
    let repository = Repository {
        root: directory.0.clone(),
    };

    let (urls, warnings) = repository.remote_urls().unwrap();

    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(urls.len(), 3);
    assert!(urls.iter().any(|entry| {
        entry.remote == "origin" && entry.url == "git@github.com:octocat/widget.git"
    }));
    assert!(urls.iter().any(|entry| entry.remote == "upstream"));
}

#[test]
fn decodes_gh_tsv_escapes_without_corrupting_literal_backslashes() {
    assert_eq!(
        unescape_tsv(r"line one\nline two\tpath\\file\q"),
        "line one\nline two\tpath\\file\\q"
    );
    assert_eq!(
        parse_tsv_record::<3>(b"one\ttwo\\tinside\tthree\r").unwrap(),
        [
            "one".to_owned(),
            "two\tinside".to_owned(),
            "three".to_owned()
        ]
    );
}

#[test]
fn derives_standard_github_repository_identity_without_network_resolution() {
    assert_eq!(
        repository_from_remote_url("https://github.com/acme/widget.git"),
        Some(GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: Vec::new(),
        })
    );
    assert!(
        repository_from_remote_url("git@github.example.com:acme/widget.git").is_none(),
        "enterprise hosts must still be validated through gh"
    );
    assert!(repository_from_remote_url("https://gitlab.com/acme/widget.git").is_none());
    assert!(repository_from_remote_url("file:///tmp/widget.git").is_none());
    assert!(repository_from_remote_url("https://github.com/acme/widget/extra").is_none());
}

#[test]
fn strips_credentials_before_passing_remote_urls_to_gh() {
    assert_eq!(
        remote_url_for_gh("https://user:secret@github.com/acme/widget.git?token=secret"),
        "https://github.com/acme/widget.git"
    );
    assert_eq!(
        remote_url_for_gh("ssh://deploy-key@github.example.com/acme/widget.git"),
        "ssh://github.example.com/acme/widget.git"
    );
    assert_eq!(
        remote_url_for_gh("token-user@github.com:acme/widget.git"),
        "ssh://github.com/acme/widget.git"
    );
}

#[test]
fn parses_cross_repository_pull_requests_with_oids() {
    let upstream = repository(
        "acme/widget",
        "https://github.com/acme/widget",
        &["upstream"],
    );
    let fork = repository(
        "octocat/widget",
        "https://github.com/octocat/widget",
        &["origin", "publish"],
    );
    let output = b"42\tShip the rocket\tDetailed\\nbody\toctocat\tOPEN\ttrue\t2026-08-13T12:00:00Z\thttps://github.com/acme/widget/pull/42\tmain\tfeature/rocket\toctocat/widget\ttrue\t12\t3\t4\tbaseoid\theadid\t2026-08-01T09:00:00Z\n";

    let requests = parse_pull_requests(output, &upstream, &[upstream.clone(), fork]).unwrap();

    let request = &requests[0];
    assert_eq!(request.description, "Detailed\nbody");
    assert_eq!(request.base_label(), "acme/widget:main");
    assert_eq!(request.head_label(), "octocat/widget:feature/rocket");
    assert_eq!(request.head_remotes, vec!["origin", "publish"]);
    assert_eq!(request.base_oid, "baseoid");
    assert_eq!(request.head_oid, "headid");
    assert!(request.is_cross_repository);
}

#[test]
fn deleted_fork_metadata_uses_the_base_repository_pr_ref() {
    let base = repository(
        "acme/widget",
        "https://github.example.com/acme/widget",
        &["enterprise"],
    );
    let output = b"7\tOld contribution\t\tghost\tOPEN\tfalse\t2026-01-01T00:00:00Z\thttps://github.example.com/acme/widget/pull/7\ttrunk\tlost-branch\t\tfalse\t0\t0\t1\tbaseoid\theadid\t2025-12-30T00:00:00Z\n";

    let request = parse_pull_requests(output, &base, std::slice::from_ref(&base))
        .unwrap()
        .remove(0);

    assert_eq!(request.author, "ghost");
    assert_eq!(request.head_label(), "deleted fork:lost-branch");
    assert!(request.head_repository.is_none());
    assert_eq!(request.head_remotes, Vec::<String>::new());
    assert_eq!(request.base_repository.host(), "github.example.com");
}

#[test]
fn exact_lookup_command_is_repository_scoped_and_requests_oids() {
    let repository = repository(
        "acme/widget",
        "https://github.example.com/acme/widget",
        &["work"],
    );
    let args = pull_request_view_args(&repository, 19);
    let args = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        &args[..5],
        &["pr", "view", "19", "--repo", repository.url.as_str()]
    );
    assert!(args.iter().any(|arg| arg.contains("baseRefOid")));
    assert!(args.iter().any(|arg| arg.contains("headRefOid")));
    assert!(args.iter().any(|arg| arg.contains("body")));
}

#[test]
fn cache_round_trips_private_metadata_and_uses_stable_keys() {
    let directory = test_directory("cache");
    let cache = CacheStore::at(directory.0.clone());
    cache
        .write("repo\npage 1", b"metadata\n", MAX_GH_METADATA_BYTES)
        .unwrap();

    let entry = cache.read("repo\npage 1", MAX_GH_METADATA_BYTES).unwrap();
    assert_eq!(entry.data, b"metadata\n");
    assert!(entry.age < Duration::from_secs(2));
    assert_eq!(cache.path("same"), cache.path("same"));
    assert_ne!(cache.path("same"), cache.path("different"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(cache.path("repo\npage 1"))
                .unwrap()
                .permissions()
                .mode()
                & 0o077,
            0
        );
    }
}

#[test]
fn cached_pull_request_metadata_becomes_a_recent_entry() {
    let directory = test_directory("recent-cache");
    let cache = CacheStore::at(directory.0.clone());
    let record = b"39\tRestore selectable previews\tDetails\toctocat\tOPEN\tfalse\t2026-08-18T05:35:58Z\thttps://github.com/acme/widget/pull/39\tmain\tfix/previews\tacme/widget\tfalse\t12\t3\t2\tbase\thead\t2026-08-17T16:35:45Z\n";
    cache
        .write("pull request", record, MAX_GH_METADATA_BYTES)
        .unwrap();

    let recent = cache.cached_pull_requests();

    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].number, 39);
    assert_eq!(recent[0].title, "Restore selectable previews");
    assert_eq!(recent[0].repository.name_with_owner, "acme/widget");
    assert_eq!(recent[0].repository.url, "https://github.com/acme/widget");
}

#[test]
fn selected_file_counts_include_raw_patch_lines_when_rendering_is_truncated() {
    let base = repository("acme/widget", "https://github.com/acme/widget", &["origin"]);
    let mut request = pull_request(base, 9);
    request.changed_files = 1;
    request.additions = 3;
    request.deletions = 2;
    let patch = b"diff --git a/test.txt b/test.txt\n--- a/test.txt\n+++ b/test.txt\n@@ -1,2 +1,3 @@\n-old one\n-old two\n+new one\n+new two\n+new three\n";

    let file = PullRequestFile {
        path: PathBuf::from("test.txt"),
        old_path: None,
        status: PullRequestFileStatus::Modified,
        counts: None,
    };
    let document = pull_request_file_document(patch, &request, &file, true);
    let details = document.pull_request_details.unwrap();

    assert_eq!(
        (
            details.selected_file_additions,
            details.selected_file_deletions
        ),
        (3, 2)
    );
    assert_eq!((details.additions, details.deletions), (3, 2));
}

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

#[test]
fn a_response_head_is_read_apart_from_its_body() {
    let response = b"HTTP/2.0 200 OK\r\nEtag: W/\"92ade\"\r\nContent-Type: application/json\r\n\r\n[{\"a\":1}]";
    let (head, body) = split_http_response(response);
    assert!(head.starts_with("HTTP/2.0 200 OK"));
    assert_eq!(body, b"[{\"a\":1}]");
    assert_eq!(header_value(&head, "etag").as_deref(), Some("W/\"92ade\""));
    assert_eq!(header_value(&head, "ETAG").as_deref(), Some("W/\"92ade\""));
    assert_eq!(header_value(&head, "link"), None);
}

#[test]
fn a_body_the_head_cannot_describe_still_arrives_whole() {
    let mut response = b"HTTP/2.0 200 OK\n\n".to_vec();
    response.extend_from_slice(&[0xff, 0xfe, b'o', b'k']);
    let (head, body) = split_http_response(&response);
    assert_eq!(head, "HTTP/2.0 200 OK");
    assert_eq!(body, [0xff, 0xfe, b'o', b'k']);
}

#[test]
fn only_a_single_page_answer_is_worth_a_validator() {
    let paged = "HTTP/2.0 200 OK\nLink: <https://api.github.com/x?page=2>; rel=\"next\"";
    let last = "HTTP/2.0 200 OK\nLink: <https://api.github.com/x?page=1>; rel=\"prev\"";
    assert!(has_next_page(paged));
    assert!(!has_next_page(last));
    assert!(!has_next_page("HTTP/2.0 200 OK"));
}

#[test]
fn api_file_counts_parse_and_skip_malformed_records() {
    let data = b"src/main.rs\t12\t3\tmodified\nREADME.md\t1\t0\tmodified\nbroken record\nassets/logo.png\tnot\tnumbers\tadded\nassets/icon.png\t0\t0\tadded\nsrc/old_name.rs\t0\t0\trenamed\n";
    let counts = parse_api_file_counts(data);

    assert_eq!(
        counts.len(),
        3,
        "malformed and countless records are skipped, pure renames are kept"
    );
    assert_eq!(
        counts.get(Path::new("src/old_name.rs")),
        Some(&DiffLineCounts {
            additions: 0,
            deletions: 0,
            binary: false,
        }),
        "a pure rename really has zero changed lines"
    );
    assert_eq!(
        counts.get(Path::new("src/main.rs")),
        Some(&DiffLineCounts {
            additions: 12,
            deletions: 3,
            binary: false,
        })
    );
}

#[test]
fn the_link_header_names_the_newest_timeline_page() {
    let head = "HTTP/2.0 200 OK\nLink: <https://api.github.com/x?per_page=100&page=2>; rel=\"next\", <https://api.github.com/x?per_page=100&page=12>; rel=\"last\"";
    assert_eq!(last_page(head), Some(12));
    assert_eq!(
        last_page("HTTP/2.0 200 OK"),
        None,
        "a single page advertises no last page"
    );
    let reversed =
        "HTTP/2.0 200 OK\nLink: <https://api.github.com/x?page=7&per_page=100>; rel=\"last\"";
    assert_eq!(
        last_page(reversed),
        Some(7),
        "per_page never shadows the page parameter"
    );
}

#[test]
fn a_cache_entry_keeps_its_validator_beside_the_body_it_validates() {
    let entry = b"W/\"92ade\"\nname\tvalue\n";
    let (validator, body) = split_validator(entry);
    assert_eq!(validator.as_deref(), Some("W/\"92ade\""));
    assert_eq!(body, b"name\tvalue\n");

    let (missing, whole) = split_validator(b"no newline here");
    assert_eq!(missing, None);
    assert_eq!(whole, b"no newline here");
}
