use super::*;

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
        mode: PullRequestMergeMode::Direct,
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
fn pull_request_operations_map_to_non_interactive_gh_commands() {
    let mut pull_request = pull_request(
        repository(
            "pulkitxm/quinjet",
            "https://github.com/pulkitxm/quinjet",
            &[],
        ),
        12,
    );
    pull_request.head_oid = "abc123".to_owned();
    pull_request.action_state.node_id = "PR_node".to_owned();
    let args = |operation: &PullRequestOperation| {
        api::pull_request_operation_args(&pull_request, operation)
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        args(&PullRequestOperation::Merge {
            method: PullRequestMergeMethod::Squash,
            mode: PullRequestMergeMode::Auto,
            delete_branch: true,
        }),
        [
            "pr",
            "merge",
            "12",
            "--repo",
            "https://github.com/pulkitxm/quinjet",
            "--squash",
            "--auto",
            "--match-head-commit",
            "abc123",
            "--delete-branch",
        ]
    );
    assert!(args(&PullRequestOperation::SetDraft(true)).ends_with(&["--undo".to_owned()]));
    assert!(
        args(&PullRequestOperation::Review {
            kind: PullRequestReviewKind::RequestChanges,
            body: "Needs tests".to_owned(),
        })
        .ends_with(&[
            "--request-changes".to_owned(),
            "--body".to_owned(),
            "Needs tests".to_owned(),
        ])
    );
    assert!(
        args(&PullRequestOperation::Edit(PullRequestEdit::AddReviewer(
            "octocat".to_owned(),
        )))
        .ends_with(&["--add-reviewer".to_owned(), "octocat".to_owned()])
    );
    assert!(
        args(&PullRequestOperation::Dequeue)
            .iter()
            .any(|arg| arg.contains("dequeuePullRequest"))
    );
    assert!(
        args(&PullRequestOperation::Subscribe(false))
            .iter()
            .any(|arg| arg == "state=UNSUBSCRIBED")
    );
    assert!(
        args(&PullRequestOperation::Comment {
            mode: PullRequestCommentMode::DeleteLast,
            body: String::new(),
        })
        .ends_with(&["--delete-last".to_owned(), "--yes".to_owned()])
    );
    assert!(
        args(&PullRequestOperation::SetMaintainerEdits(true))
            .iter()
            .any(|arg| arg == "enabled=true")
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
    let output = b"PR_node\t42\tShip the rocket\tDetailed\\nbody\toctocat\tOPEN\ttrue\t2026-08-13T12:00:00Z\thttps://github.com/acme/widget/pull/42\tmain\tfeature/rocket\toctocat/widget\ttrue\t12\t3\t4\tbaseoid\theadid\t2026-08-01T09:00:00Z\tfalse\ttrue\tfalse\ttrue\ttrue\ttrue\ttrue\ttrue\tSUBSCRIBED\tCLEAN\tMERGEABLE\tfalse\ttrue\t\t\t0\t\t\tAPPROVED\n";

    let requests = parse_pull_requests(output, &upstream, &[upstream.clone(), fork]).unwrap();

    let request = &requests[0];
    assert_eq!(request.description, "Detailed\nbody");
    assert_eq!(request.base_label(), "acme/widget:main");
    assert_eq!(request.head_label(), "octocat/widget:feature/rocket");
    assert_eq!(request.head_remotes, vec!["origin", "publish"]);
    assert_eq!(request.base_oid, "baseoid");
    assert_eq!(request.head_oid, "headid");
    assert!(request.is_cross_repository);
    assert_eq!(request.action_state.node_id, "PR_node");
    assert!(request.action_state.viewer_can_update_branch);
    assert_eq!(request.action_state.review_decision, "APPROVED");
}

#[test]
fn deleted_fork_metadata_uses_the_base_repository_pr_ref() {
    let base = repository(
        "acme/widget",
        "https://github.example.com/acme/widget",
        &["enterprise"],
    );
    let output = b"PR_deleted\t7\tOld contribution\t\tghost\tOPEN\tfalse\t2026-01-01T00:00:00Z\thttps://github.example.com/acme/widget/pull/7\ttrunk\tlost-branch\t\tfalse\t0\t0\t1\tbaseoid\theadid\t2025-12-30T00:00:00Z\tfalse\ttrue\tfalse\ttrue\tfalse\ttrue\ttrue\ttrue\tSUBSCRIBED\tUNKNOWN\tUNKNOWN\tfalse\ttrue\t\t\t0\t\t\t\n";

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
        &args[..4],
        &["api", "graphql", "--hostname", "github.example.com"]
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
    let record = b"PR_recent\t39\tRestore selectable previews\tDetails\toctocat\tOPEN\tfalse\t2026-08-18T05:35:58Z\thttps://github.com/acme/widget/pull/39\tmain\tfix/previews\tacme/widget\tfalse\t12\t3\t2\tbase\thead\t2026-08-17T16:35:45Z\tfalse\ttrue\tfalse\ttrue\ttrue\ttrue\ttrue\ttrue\tSUBSCRIBED\tCLEAN\tMERGEABLE\tfalse\ttrue\t\t\t0\t\t\t\n";
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
