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
