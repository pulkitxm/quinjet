use super::*;

fn operation_pull_request() -> PullRequest {
    let mut request = pull_request(
        repository(
            "pulkitxm/quinjet",
            "https://github.com/pulkitxm/quinjet",
            &[],
        ),
        12,
    );
    request.head_oid = "abc123".into();
    request.action_state.node_id = "PR_node".into();
    request
}

fn operation_arguments(request: &PullRequest, operation: &PullRequestOperation) -> Vec<String> {
    api::pull_request_operation_args(request, operation)
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect()
}

fn contract_fields(spec: &str) -> (&str, &str, &str, &str) {
    let mut fields = spec.split('|');
    let values = (
        fields.next().unwrap_or_default(),
        fields.next().unwrap_or_default(),
        fields.next().unwrap_or_default(),
        fields.next().unwrap_or_default(),
    );
    assert!(fields.next().is_none(), "invalid contract: {spec}");
    values
}

fn assert_operation_contract(
    request: &PullRequest,
    operation: &PullRequestOperation,
    spec: &str,
    confirm: Option<&str>,
) {
    let (label, success, transport, required) = contract_fields(spec);
    let generated_confirm = format!("Really {}", label.to_lowercase());
    let confirm = confirm.unwrap_or(&generated_confirm);
    assert_eq!(operation.label(), label);
    assert_eq!(operation.confirm_title(), format!("{label}?"));
    assert_eq!(
        operation.confirm_message(request),
        format!("{confirm} #12 (Ship the rocket)?")
    );
    assert_eq!(operation.success_message(request), format!("{success} #12"));
    let arguments = operation_arguments(request, operation);
    if let Some(query) = transport.strip_prefix("graphql:") {
        assert_eq!(
            &arguments[..4],
            &["api", "graphql", "--hostname", "github.com"]
        );
        assert!(arguments.iter().any(|value| value == "id=PR_node"));
        assert!(arguments.iter().any(|value| value.contains(query)));
    } else {
        assert_eq!(
            &arguments[..5],
            &[
                "pr",
                transport,
                "12",
                "--repo",
                "https://github.com/pulkitxm/quinjet"
            ]
        );
    }
    for expected in required.split(',').filter(|value| !value.is_empty()) {
        let (present, token) = expected
            .strip_prefix('!')
            .map_or((true, expected), |value| (false, value));
        assert_eq!(
            arguments.iter().any(|value| value == token),
            present,
            "{operation:?}: {token}: {arguments:?}"
        );
    }
}

fn assert_standard_contract(request: &PullRequest, operation: &PullRequestOperation, spec: &str) {
    assert_operation_contract(request, operation, spec, None);
}

macro_rules! operation_contracts {
    ($request:expr; $($operation:expr => $spec:literal;)*) => {
        $(assert_standard_contract($request, &$operation, $spec);)*
    };
}

fn review(kind: PullRequestReviewKind) -> PullRequestOperation {
    PullRequestOperation::Review {
        kind,
        body: "review body".into(),
    }
}

fn comment(mode: PullRequestCommentMode, body: &str) -> PullRequestOperation {
    PullRequestOperation::Comment {
        mode,
        body: body.into(),
    }
}

#[test]
fn every_merge_mode_and_method_has_a_stable_contract() {
    let request = operation_pull_request();
    let methods = [
        "Create a merge commit|create a merge commit for|Merged",
        "Squash and merge|squash and merge|Squashed and merged",
        "Rebase and merge|rebase and merge|Rebased and merged",
    ];
    for (method_index, (method, words)) in
        PullRequestMergeMethod::ALL.iter().zip(methods).enumerate()
    {
        let (method_label, action, method_success, _) = contract_fields(words);
        for (mode_index, mode) in [
            PullRequestMergeMode::Direct,
            PullRequestMergeMode::Auto,
            PullRequestMergeMode::Admin,
        ]
        .iter()
        .enumerate()
        {
            let (label, prefix, success) = match mode {
                PullRequestMergeMode::Direct => (method_label, "", method_success),
                PullRequestMergeMode::Auto => (
                    "Enable auto-merge",
                    "enable auto-merge to ",
                    "Enabled auto-merge for",
                ),
                PullRequestMergeMode::Admin => (
                    "Merge with administrator privileges",
                    "use administrator privileges to ",
                    "Administrator-merged",
                ),
            };
            let delete_branch = (method_index + mode_index) % 2 == 0;
            let operation = PullRequestOperation::Merge {
                method: *method,
                mode: *mode,
                delete_branch,
            };
            let spec = format!(
                "{label}|{success}|merge|{},--match-head-commit,abc123",
                method.flag()
            );
            let confirm = format!("Really {prefix}{action}");
            assert_operation_contract(&request, &operation, &spec, Some(&confirm));
            let arguments = operation_arguments(&request, &operation);
            for candidate in PullRequestMergeMethod::ALL {
                assert_eq!(
                    arguments.iter().any(|value| value == candidate.flag()),
                    candidate == *method
                );
            }
            assert_eq!(
                arguments.iter().any(|value| value == "--auto"),
                *mode == PullRequestMergeMode::Auto
            );
            assert_eq!(
                arguments.iter().any(|value| value == "--admin"),
                *mode == PullRequestMergeMode::Admin
            );
            assert_eq!(
                arguments.iter().any(|value| value == "--delete-branch"),
                delete_branch
            );
        }
    }
}

#[test]
fn state_review_and_comment_operations_have_stable_contracts() {
    use PullRequestCommentMode::{Create, DeleteLast, EditLast};
    use PullRequestReviewKind::{Approve, Comment, RequestChanges};
    let request = operation_pull_request();
    operation_contracts!(&request;
        PullRequestOperation::SetDraft(true) => "Convert to draft|Converted to draft|ready|--undo";
        PullRequestOperation::SetDraft(false) => "Mark ready for review|Marked ready for review|ready|!--undo";
        review(Approve) => "Approve pull request|Approved|review|--approve,--body,review body";
        review(Comment) => "Submit review comment|Reviewed|review|--comment,--body,review body";
        review(RequestChanges) => "Request changes|Requested changes on|review|--request-changes,--body,review body";
        comment(Create, "created") => "Comment on pull request|Commented on|comment|--body,created,!--edit-last,!--delete-last";
        comment(EditLast, "edited") => "Edit last comment|Edited the last comment on|comment|--edit-last,--body,edited";
        comment(DeleteLast, "") => "Delete last comment|Deleted the last comment on|comment|--delete-last,--yes";
    );
}

#[test]
fn every_edit_field_has_a_stable_contract() {
    use PullRequestEdit::*;
    let request = operation_pull_request();
    operation_contracts!(&request;
        PullRequestOperation::Edit(Title("New title".into())) => "Edit pull-request title|Updated|edit|--title,New title";
        PullRequestOperation::Edit(Body("New body".into())) => "Edit pull-request description|Updated|edit|--body,New body";
        PullRequestOperation::Edit(Base("trunk".into())) => "Change base branch|Updated|edit|--base,trunk";
        PullRequestOperation::Edit(AddAssignee("octocat".into())) => "Add assignees|Updated|edit|--add-assignee,octocat";
        PullRequestOperation::Edit(RemoveAssignee("hubot".into())) => "Remove assignees|Updated|edit|--remove-assignee,hubot";
        PullRequestOperation::Edit(AddLabel("bug".into())) => "Add labels|Updated|edit|--add-label,bug";
        PullRequestOperation::Edit(RemoveLabel("stale".into())) => "Remove labels|Updated|edit|--remove-label,stale";
        PullRequestOperation::Edit(AddProject("Roadmap".into())) => "Add to projects|Updated|edit|--add-project,Roadmap";
        PullRequestOperation::Edit(RemoveProject("Backlog".into())) => "Remove from projects|Updated|edit|--remove-project,Backlog";
        PullRequestOperation::Edit(AddReviewer("reviewer".into())) => "Request reviewers|Updated|edit|--add-reviewer,reviewer";
        PullRequestOperation::Edit(RemoveReviewer("former".into())) => "Remove review requests|Updated|edit|--remove-reviewer,former";
        PullRequestOperation::Edit(SetMilestone("v1".into())) => "Set milestone|Updated|edit|--milestone,v1";
        PullRequestOperation::Edit(RemoveMilestone) => "Remove milestone|Updated|edit|--remove-milestone";
    );
}

#[test]
fn branch_and_conversation_operations_have_stable_contracts() {
    use PullRequestLockReason::{OffTopic, Resolved, Spam, TooHeated};
    let request = operation_pull_request();
    operation_contracts!(&request;
        PullRequestOperation::UpdateBranch(PullRequestUpdateMethod::Merge) => "Update branch with merge|Updated the branch for|update-branch|!--rebase";
        PullRequestOperation::UpdateBranch(PullRequestUpdateMethod::Rebase) => "Update branch with rebase|Updated the branch for|update-branch|--rebase";
        PullRequestOperation::DisableAutoMerge => "Disable auto-merge|Disabled auto-merge for|merge|--disable-auto";
        PullRequestOperation::Dequeue => "Remove from merge queue|Removed from the merge queue|graphql:dequeuePullRequest|";
        PullRequestOperation::Lock(None) => "Lock conversation|Locked the conversation on|lock|!--reason";
        PullRequestOperation::Lock(Some(OffTopic)) => "Lock conversation|Locked the conversation on|lock|--reason,off_topic";
        PullRequestOperation::Lock(Some(Resolved)) => "Lock conversation|Locked the conversation on|lock|--reason,resolved";
        PullRequestOperation::Lock(Some(Spam)) => "Lock conversation|Locked the conversation on|lock|--reason,spam";
        PullRequestOperation::Lock(Some(TooHeated)) => "Lock conversation|Locked the conversation on|lock|--reason,too_heated";
        PullRequestOperation::Unlock => "Unlock conversation|Unlocked the conversation on|unlock|";
    );
}

#[test]
fn subscription_maintainer_revert_and_state_operations_have_stable_contracts() {
    let request = operation_pull_request();
    operation_contracts!(&request;
        PullRequestOperation::Subscribe(true) => "Subscribe to pull request|Subscribed to|graphql:updateSubscription|state=SUBSCRIBED";
        PullRequestOperation::Subscribe(false) => "Unsubscribe from pull request|Unsubscribed from|graphql:updateSubscription|state=UNSUBSCRIBED";
        PullRequestOperation::SetMaintainerEdits(true) => "Allow maintainer edits|Allowed maintainer edits on|graphql:updatePullRequest|enabled=true";
        PullRequestOperation::SetMaintainerEdits(false) => "Disallow maintainer edits|Disallowed maintainer edits on|graphql:updatePullRequest|enabled=false";
        PullRequestOperation::Revert { draft: false, title: String::new(), body: String::new() } => "Create revert pull request|Created a revert for|revert|!--draft,!--title,!--body";
        PullRequestOperation::Revert { draft: true, title: "Undo launch".into(), body: "Rollback".into() } => "Create revert pull request|Created a revert for|revert|--draft,--title,Undo launch,--body,Rollback";
    );
    assert_operation_contract(
        &request,
        &PullRequestOperation::Close,
        "Close pull request|Closed|close|",
        Some("Really close"),
    );
    assert_operation_contract(
        &request,
        &PullRequestOperation::Reopen,
        "Reopen pull request|Reopened|reopen|",
        Some("Really reopen"),
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
        github_cli: None,
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
