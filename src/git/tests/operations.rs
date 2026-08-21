use super::*;

#[test]
fn compares_head_with_another_branch_without_checkout() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    run_test_git(&test_repository.path, ["switch", "-c", "topic"]);
    fs::write(test_repository.path.join("topic.txt"), "topic\n").unwrap();
    fs::write(test_repository.path.join("second.txt"), "second\n").unwrap();
    run_test_git(&test_repository.path, ["add", "topic.txt", "second.txt"]);
    run_test_git(
        &test_repository.path,
        [
            "-c",
            "user.name=Quinjet Test",
            "-c",
            "user.email=quinjet@example.com",
            "commit",
            "--message=topic",
        ],
    );
    run_test_git(&test_repository.path, ["switch", "main"]);

    let prepared = repository
        .prepare_local_diff(&LocalDiffRequest::Branch {
            branch: Box::new(HistoryBranch {
                name: "topic".to_owned(),
                reference: "refs/heads/topic".to_owned(),
                current: false,
                remote: false,
                relative_date: "now".to_owned(),
                short_id: "abcdef0".to_owned(),
            }),
            current: "main".to_owned(),
            current_oid: None,
            expanded: false,
        })
        .unwrap();
    let index = prepared.index();
    assert_eq!(index.files.len(), 2);
    assert_eq!(
        index
            .files
            .iter()
            .map(|file| file.counts)
            .collect::<Vec<_>>(),
        vec![
            Some(DiffLineCounts {
                additions: 0,
                deletions: 1,
                binary: false,
            });
            2
        ],
        "a branch index must know every file's totals before any patch is read"
    );
    let document = prepared.diff_file(&index.files[0].path).unwrap();

    assert!(document.title.contains("topic"));
    assert_eq!(
        document.file_count(),
        1,
        "only the selected path is patched"
    );
    assert!(document.lines.iter().any(|line| {
        line.text()
            .contains(index.files[0].path.to_string_lossy().as_ref())
    }));
    assert_eq!(
        run_test_git(&test_repository.path, ["branch", "--show-current"]),
        "main"
    );
}

#[test]
fn creates_lists_previews_applies_and_drops_stashes() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    fs::write(test_repository.path.join("README.md"), "stashed\n").unwrap();
    fs::write(test_repository.path.join("untracked.txt"), "also stashed\n").unwrap();

    repository
        .perform(&GitOperation::StashPush {
            message: "save launch work".to_owned(),
            include_untracked: true,
            staged: false,
            paths: Vec::new(),
        })
        .unwrap();
    assert_eq!(repository.status().unwrap().changes, []);

    let stashes = repository.stashes().unwrap();
    assert_eq!(stashes.len(), 1);
    assert_eq!(stashes[0].reference, "stash@{0}");
    assert_eq!(stashes[0].message, "save launch work");
    assert_eq!(stashes[0].branch, "main");
    let prepared = repository
        .prepare_local_diff(&LocalDiffRequest::Stash {
            stash: Box::new(stashes[0].clone()),
            expanded: false,
        })
        .unwrap();
    let index = prepared.index();
    assert_eq!(index.files.len(), 2);
    assert_eq!(
        prepared
            .diff_file(&index.files[0].path)
            .unwrap()
            .file_count(),
        1
    );

    repository
        .perform(&GitOperation::StashApply(stashes[0].reference.clone()))
        .unwrap();
    assert_ne!(repository.status().unwrap().changes, []);
    run_test_git(&test_repository.path, ["reset", "--hard", "HEAD"]);
    run_test_git(&test_repository.path, ["clean", "-fd"]);
    repository
        .perform(&GitOperation::StashDrop(stashes[0].reference.clone()))
        .unwrap();
    assert_eq!(repository.stashes().unwrap(), []);
}

#[test]
fn tracked_only_stash_preview_does_not_require_an_untracked_parent() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    fs::write(
        test_repository.path.join("index2.ts"),
        "const name = \"Test\";\n\nconsole.log(name + \"!\")\n",
    )
    .unwrap();
    run_test_git(&test_repository.path, ["add", "index2.ts"]);

    repository
        .perform(&GitOperation::StashPush {
            message: "tracked addition".to_owned(),
            include_untracked: false,
            staged: false,
            paths: Vec::new(),
        })
        .unwrap();
    let stash = repository.stashes().unwrap().remove(0);
    let prepared = repository
        .prepare_local_diff(&LocalDiffRequest::Stash {
            stash: Box::new(stash),
            expanded: false,
        })
        .unwrap();
    let index = prepared.index();
    assert_eq!(index.files.len(), 1);

    let document = prepared.diff_file(&index.files[0].path).unwrap();
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.text().contains("const name"))
    );
}

#[test]
fn staged_only_stash_leaves_unstaged_worktree_changes_in_place() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    fs::write(test_repository.path.join("other.txt"), "base\n").unwrap();
    run_test_git(&test_repository.path, ["add", "other.txt"]);
    run_test_git(
        &test_repository.path,
        [
            "-c",
            "user.name=Quinjet Test",
            "-c",
            "user.email=quinjet@example.com",
            "commit",
            "--message=track other",
        ],
    );
    fs::write(test_repository.path.join("README.md"), "staged\n").unwrap();
    fs::write(test_repository.path.join("other.txt"), "unstaged\n").unwrap();
    run_test_git(&test_repository.path, ["add", "README.md"]);

    repository
        .perform(&GitOperation::StashPush {
            message: "index only".to_owned(),
            include_untracked: false,
            staged: true,
            paths: Vec::new(),
        })
        .unwrap();

    assert_eq!(
        fs::read_to_string(test_repository.path.join("README.md"))
            .unwrap()
            .trim_end(),
        "test repository"
    );
    assert_eq!(
        fs::read_to_string(test_repository.path.join("other.txt"))
            .unwrap()
            .trim_end(),
        "unstaged"
    );
    let status = repository.status().unwrap();
    assert_eq!(status.staged_count(), 0);
    assert!(status.changes.iter().any(|change| {
        change.path == Path::new("other.txt") && change.area == ChangeArea::Unstaged
    }));
    assert_eq!(repository.stashes().unwrap()[0].message, "index only");
}

#[test]
fn branch_rename_rejects_invalid_identical_and_existing_names() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    run_test_git(&test_repository.path, ["branch", "existing"]);

    for new in ["main", "bad..name", "existing"] {
        assert!(
            repository
                .perform(&GitOperation::RenameBranch {
                    old: "main".to_owned(),
                    new: new.to_owned(),
                })
                .is_err(),
            "rename to {new:?} should fail"
        );
    }
    assert_eq!(
        run_test_git(&test_repository.path, ["branch", "--show-current"]),
        "main"
    );
}

#[test]
fn parses_porcelain_worktrees_and_marks_the_session_root() {
    let output = b"worktree /tmp/repo\0HEAD abcdef0123456789\0branch refs/heads/main\0\0worktree /tmp/repo-topic\0HEAD fedcba9876543210\0branch refs/heads/topic\0locked busy\0\0worktree /tmp/repo-hot\0HEAD 0123456789abcdef\0detached\0prunable\0\0";
    let trees = parse_worktrees(output, Path::new("/tmp/repo"));
    assert_eq!(trees.len(), 3);
    assert!(trees[0].current);
    assert_eq!(trees[0].branch.as_deref(), Some("main"));
    assert!(!trees[1].current);
    assert_eq!(trees[1].branch.as_deref(), Some("topic"));
    assert_eq!(trees[1].locked.as_deref(), Some("busy"));
    assert!(trees[2].detached);
    assert!(trees[2].branch.is_none());
    assert_eq!(trees[2].prunable.as_deref(), Some(""));
}

#[cfg(windows)]
#[test]
fn git_worktree_paths_use_native_separators() {
    assert_eq!(
        parse_worktree_path(br"C:/Users/runner/work"),
        PathBuf::from(r"C:\Users\runner\work")
    );
}

#[test]
fn lists_a_linked_worktree_without_changing_head() {
    let test_repository = TestRepository::new();
    let repository = test_repository.repository();
    let linked_root = tempfile::tempdir().unwrap();
    let linked = linked_root.path().join("topic");
    let linked_display = linked.display().to_string();
    run_test_git(
        &test_repository.path,
        ["worktree", "add", "-b", "topic", &linked_display],
    );
    let trees = repository.worktrees().unwrap();
    assert_eq!(trees.len(), 2);
    assert!(
        trees
            .iter()
            .any(|tree| tree.current && tree.branch.as_deref() == Some("main"))
    );
    assert!(trees.iter().any(|tree| {
        !tree.current && tree.branch.as_deref() == Some("topic") && same_path(&tree.path, &linked)
    }));
    assert_eq!(
        run_test_git(&test_repository.path, ["branch", "--show-current"]),
        "main"
    );
    drop(fs::remove_dir_all(&linked));
    run_test_git(&test_repository.path, ["worktree", "prune"]);
}
