use super::*;

#[test]
fn rejects_paths_outside_worktree() {
    let root = Path::new("/tmp/repository");
    safe_worktree_path(root, Path::new("../secret")).unwrap_err();
    safe_worktree_path(root, Path::new("/etc/passwd")).unwrap_err();
    assert_eq!(
        safe_worktree_path(root, Path::new("src/main.rs")).unwrap(),
        PathBuf::from("/tmp/repository/src/main.rs")
    );
}

#[test]
fn truncates_at_line_boundary() {
    let mut input = b"first\nsecond\nthird\n".to_vec();
    assert!(truncate(&mut input, 15));
    assert_eq!(input, b"first\nsecond\n");
}

#[test]
fn reads_a_selected_branch_history_without_changing_head_or_worktree() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    run_test_git(&test_repository.path, ["switch", "-c", "topic"]);
    fs::write(test_repository.path.join("topic.txt"), "topic\n").unwrap();
    run_test_git(&test_repository.path, ["add", "topic.txt"]);
    run_test_git(
        &test_repository.path,
        [
            "-c",
            "user.name=Quinjet Test",
            "-c",
            "user.email=quinjet@example.com",
            "commit",
            "--message=topic commit",
        ],
    );
    let topic_id = run_test_git(&test_repository.path, ["rev-parse", "HEAD"]);
    run_test_git(&test_repository.path, ["switch", "main"]);
    let refs_before = run_test_git(&test_repository.path, ["show-ref"]);

    let main = repository.history("HEAD", 0, 50).unwrap();
    let topic = repository.history("refs/heads/topic", 0, 50).unwrap();

    assert!(!main.iter().any(|commit| commit.id == topic_id));
    assert!(topic.iter().any(|commit| commit.id == topic_id));
    assert_eq!(
        run_test_git(&test_repository.path, ["branch", "--show-current"]),
        "main"
    );
    assert_eq!(
        run_test_git(&test_repository.path, ["status", "--porcelain"]),
        ""
    );
    assert_eq!(
        run_test_git(&test_repository.path, ["show-ref"]),
        refs_before
    );
    repository.history("--all", 0, 50).unwrap_err();
}

#[test]
fn lists_history_branches_with_full_safe_references() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    run_test_git(&test_repository.path, ["branch", "topic"]);
    run_test_git(
        &test_repository.path,
        ["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    run_test_git(
        &test_repository.path,
        [
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    );

    let branches = repository.history_branches().unwrap();

    assert!(branches.iter().any(|branch| {
        branch.current && branch.name == "main" && branch.reference == "refs/heads/main"
    }));
    assert!(branches.iter().any(|branch| {
        !branch.current
            && !branch.remote
            && branch.name == "topic"
            && branch.reference == "refs/heads/topic"
    }));
    assert!(branches.iter().any(|branch| {
        branch.remote
            && branch.name == "origin/main"
            && branch.reference == "refs/remotes/origin/main"
    }));
    assert!(!branches.iter().any(|branch| branch.name == "origin/HEAD"));
}

#[test]
fn renames_the_current_local_branch() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();

    let message = repository
        .perform(&GitOperation::RenameBranch {
            old: "main".to_owned(),
            new: "feature/renamed".to_owned(),
        })
        .unwrap();

    assert_eq!(message, "Renamed local branch main to feature/renamed");
    assert_eq!(
        run_test_git(&test_repository.path, ["branch", "--show-current"]),
        "feature/renamed"
    );
    assert!(
        GitOperation::RenameBranch {
            old: "main".to_owned(),
            new: "feature/renamed".to_owned(),
        }
        .changes_history()
    );
}

#[test]
fn renames_a_non_current_branch_and_preserves_its_tracking_config() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    run_test_git(&test_repository.path, ["branch", "topic"]);
    run_test_git(
        &test_repository.path,
        ["config", "branch.topic.remote", "origin"],
    );
    run_test_git(
        &test_repository.path,
        ["config", "branch.topic.merge", "refs/heads/topic"],
    );

    repository
        .perform(&GitOperation::RenameBranch {
            old: "topic".to_owned(),
            new: "feature/topic".to_owned(),
        })
        .unwrap();

    assert_eq!(
        run_test_git(&test_repository.path, ["branch", "--show-current"]),
        "main"
    );
    assert_eq!(
        run_test_git(
            &test_repository.path,
            ["config", "branch.feature/topic.remote"]
        ),
        "origin"
    );
    assert_eq!(
        run_test_git(
            &test_repository.path,
            ["config", "branch.feature/topic.merge"]
        ),
        "refs/heads/topic"
    );
    assert!(
        repository
            .run(strings([
                "show-ref",
                "--verify",
                "refs/heads/feature/topic"
            ]))
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn stages_and_unstages_one_file_without_touching_another() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    fs::write(test_repository.path.join("README.md"), "changed\n").unwrap();
    fs::write(test_repository.path.join("other.txt"), "other\n").unwrap();

    repository
        .perform(&GitOperation::Stage(vec![PathBuf::from("README.md")]))
        .unwrap();
    let status = repository.status().unwrap();
    assert!(status.changes.iter().any(|change| {
        change.path == Path::new("README.md") && change.area == ChangeArea::Staged
    }));
    assert!(status.changes.iter().any(|change| {
        change.path == Path::new("other.txt") && change.area == ChangeArea::Unstaged
    }));

    repository
        .perform(&GitOperation::Unstage(vec![PathBuf::from("README.md")]))
        .unwrap();
    let status = repository.status().unwrap();
    assert!(!status.changes.iter().any(|change| {
        change.path == Path::new("README.md") && change.area == ChangeArea::Staged
    }));
    assert_eq!(
        status
            .changes
            .iter()
            .filter(|change| change.area == ChangeArea::Unstaged)
            .count(),
        2
    );
}

#[test]
fn working_tree_index_reads_totals_for_the_area_each_change_belongs_to() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    fs::write(test_repository.path.join("staged.txt"), "one\ntwo\n").unwrap();
    run_test_git(&test_repository.path, ["add", "staged.txt"]);
    fs::write(
        test_repository.path.join("README.md"),
        "test repository\nmore\n",
    )
    .unwrap();
    fs::write(test_repository.path.join("untracked.txt"), "fresh\n").unwrap();
    let status = repository.status().unwrap();

    let prepared = repository
        .prepare_local_diff(&LocalDiffRequest::Changes {
            changes: status.changes,
            version: 0,
            expanded: false,
        })
        .unwrap();
    let index = prepared.index();
    let counts_for = |name: &str| {
        index
            .files
            .iter()
            .find(|file| file.path == Path::new(name))
            .and_then(|file| file.counts)
    };

    assert_eq!(
        counts_for("staged.txt"),
        Some(DiffLineCounts {
            additions: 2,
            deletions: 0,
            binary: false,
        })
    );
    assert_eq!(
        counts_for("README.md"),
        Some(DiffLineCounts {
            additions: 1,
            deletions: 0,
            binary: false,
        })
    );
    assert_eq!(counts_for("untracked.txt"), None);
}

#[test]
fn stash_references_and_subjects_accept_only_canonical_forms() {
    for reference in ["stash@{0}", "stash@{1}", "stash@{999999}"] {
        assert!(valid_stash_reference(reference), "{reference}");
        validate_stash_reference(reference).unwrap();
    }
    for reference in [
        "",
        "stash",
        "stash@{}",
        "stash@{-1}",
        "stash@{1a}",
        "stash@{ 1}",
        "refs/stash",
        "stash@{1}@{0}",
    ] {
        assert!(!valid_stash_reference(reference), "{reference}");
        validate_stash_reference(reference).unwrap_err();
    }
    for (subject, branch, message) in [
        ("WIP on main: save work", "main", "save work"),
        ("On feature/live: checkpoint", "feature/live", "checkpoint"),
        (" custom subject ", "", "custom subject"),
        ("WIP on missing separator", "", "WIP on missing separator"),
        ("On : empty branch", "", "empty branch"),
    ] {
        assert_eq!(
            parse_stash_subject(subject),
            (branch.to_owned(), message.to_owned())
        );
    }
}

#[test]
fn history_references_oids_and_short_ids_follow_strict_shapes() {
    for reference in [
        "refs/heads/main",
        "refs/heads/feature/live",
        "refs/remotes/origin/main",
    ] {
        validate_history_reference(reference).unwrap();
    }
    for reference in [
        "HEAD",
        "main",
        "refs/tags/v1",
        "refs/pull/1/head",
        "--all",
        "",
    ] {
        validate_history_reference(reference).unwrap_err();
    }
    let sha1 = "a".repeat(40);
    let sha256 = "A1".repeat(32);
    for oid in [&sha1, &sha256] {
        assert!(is_full_oid(oid));
        assert_eq!(short_id(oid), oid.get(..8).unwrap());
    }
    for value in ["a".repeat(39), "a".repeat(41), "g".repeat(40)] {
        assert!(!is_full_oid(&value));
    }
    assert_eq!(short_id("short"), "short");
    assert_eq!(short_id("ééééé"), "éééé");
}

#[test]
fn truncation_trimming_and_pluralization_cover_boundaries() {
    for (maximum, expected, changed) in [
        (0, b"".as_slice(), true),
        (5, b"".as_slice(), true),
        (6, b"first\n".as_slice(), true),
        (12, b"first\n".as_slice(), true),
        (13, b"first\nsecond\n".as_slice(), false),
        (99, b"first\nsecond\n".as_slice(), false),
    ] {
        let mut value = b"first\nsecond\n".to_vec();
        assert_eq!(truncate(&mut value, maximum), changed, "{maximum}");
        assert_eq!(value, expected, "{maximum}");
    }
    for (input, expected) in [
        (b"".as_slice(), b"".as_slice()),
        (b" \t\r\n".as_slice(), b"".as_slice()),
        (b"  value\n".as_slice(), b"value".as_slice()),
        (b"inside space".as_slice(), b"inside space".as_slice()),
    ] {
        assert_eq!(trim_ascii(input), expected);
    }
    assert_eq!(plural_message(0, "item", "items"), "0 items");
    assert_eq!(plural_message(1, "item", "items"), "1 item");
    assert_eq!(plural_message(2, "item", "items"), "2 items");
}

#[test]
fn diff_index_helpers_preserve_ranges_and_rename_paths() {
    let args = diff_index_args("base", "head");
    assert_eq!(
        args,
        strings([
            "diff",
            "--name-status",
            "-z",
            "--find-renames",
            "base",
            "head",
            "--"
        ])
    );
    let totals = numstat_args(&args).unwrap();
    assert_eq!(totals.get(1), Some(&OsString::from("--numstat")));
    assert_eq!(totals.get(4..), args.get(4..));
    assert!(numstat_args(&strings(["diff", "--stat"])).is_none());

    let renamed = DiffFileIndexEntry::new(
        PathBuf::from("new name.txt"),
        Some(PathBuf::from("old name.txt")),
        "renamed".to_owned(),
    );
    let mut command = vec![OsString::from("git")];
    append_diff_file_paths(&mut command, &renamed);
    assert_eq!(command, strings(["git", "old name.txt", "new name.txt"]));
    for (status, label) in [
        (b'A', "added"),
        (b'M', "modified"),
        (b'D', "deleted"),
        (b'R', "renamed"),
        (b'C', "copied"),
        (b'T', "type changed"),
        (b'U', "unmerged"),
        (b'X', "changed"),
    ] {
        assert_eq!(diff_status_label(status), label);
    }
}

#[test]
fn worktree_parser_handles_bare_detached_and_incomplete_records() {
    let output = b"worktree /bare\0bare\0\0worktree /detached\0HEAD abcdef0123456789\0detached\0locked\0prunable stale metadata\0\0HEAD missing-path\0branch refs/heads/lost\0\0worktree /topic\0HEAD fedcba9876543210\0branch refs/heads/feature/topic\0";
    let worktrees = parse_worktrees(output, Path::new("/detached"));

    assert_eq!(worktrees.len(), 3);
    assert!(worktrees[0].bare);
    assert_eq!(worktrees[0].branch_label(), "bare");
    assert!(worktrees[1].current);
    assert!(worktrees[1].detached);
    assert_eq!(worktrees[1].short_head(), "abcdef01");
    assert_eq!(worktrees[1].branch_label(), "detached");
    assert_eq!(worktrees[1].locked.as_deref(), Some(""));
    assert_eq!(worktrees[1].prunable.as_deref(), Some("stale metadata"));
    assert_eq!(worktrees[2].branch.as_deref(), Some("feature/topic"));
    assert_eq!(worktrees[2].branch_label(), "feature/topic");
}
