use super::*;
use crate::git::github::PullRequestMergeMethod;

fn assert_arguments(operation: &StackOperation, expected: &[&str]) {
    let actual: Vec<String> = operation
        .arguments()
        .into_iter()
        .map(|argument| argument.into_string().unwrap())
        .collect();
    let expected: Vec<String> = expected.iter().map(|value| (*value).to_owned()).collect();
    assert_eq!(actual, expected);
}

#[test]
fn stack_operations_build_deterministic_extension_arguments() {
    assert_arguments(
        &StackOperation::Init {
            branches: vec!["api".to_owned(), "ui".to_owned()],
            base: Some("develop".to_owned()),
        },
        &["stack", "init", "--base", "develop", "--", "api", "ui"],
    );
    assert_arguments(
        &StackOperation::Add {
            branch: "tests".to_owned(),
            all: true,
            update: false,
            message: Some("Add tests".to_owned()),
        },
        &[
            "stack",
            "add",
            "--all",
            "--message",
            "Add tests",
            "--",
            "tests",
        ],
    );
    assert_arguments(
        &StackOperation::Checkout("42".to_owned()),
        &["stack", "checkout", "--", "42"],
    );
    assert_arguments(
        &StackOperation::Modify(StackModifyAction::Continue),
        &["stack", "modify", "--continue"],
    );
    assert_arguments(
        &StackOperation::Unstack {
            stack: Some("7".to_owned()),
            local: true,
        },
        &["stack", "unstack", "--local", "--", "7"],
    );
    assert_arguments(
        &StackOperation::Link {
            members: vec!["41".to_owned(), "42".to_owned()],
            base: Some("main".to_owned()),
            open: true,
            remote: Some("upstream".to_owned()),
        },
        &[
            "stack", "link", "--base", "main", "--open", "--remote", "upstream", "--", "41", "42",
        ],
    );
    assert_arguments(
        &StackOperation::Merge {
            target: Some("42".to_owned()),
            method: PullRequestMergeMethod::Squash,
        },
        &["stack", "merge", "--squash", "--yes", "--", "42"],
    );
    assert_arguments(
        &StackOperation::Push {
            remote: Some("origin".to_owned()),
        },
        &["stack", "push", "--remote", "origin"],
    );
    assert_arguments(
        &StackOperation::Rebase(StackRebaseAction::Start {
            branch: Some("api".to_owned()),
            downstack: true,
            upstack: false,
            no_trunk: true,
            preserve_dates: true,
            remote: Some("upstream".to_owned()),
        }),
        &[
            "stack",
            "rebase",
            "--downstack",
            "--no-trunk",
            "--committer-date-is-author-date",
            "--remote",
            "upstream",
            "--",
            "api",
        ],
    );
    assert_arguments(
        &StackOperation::Rebase(StackRebaseAction::Abort),
        &["stack", "rebase", "--abort"],
    );
    assert_arguments(
        &StackOperation::Submit {
            open: true,
            remote: Some("origin".to_owned()),
        },
        &["stack", "submit", "--auto", "--open", "--remote", "origin"],
    );
    assert_arguments(
        &StackOperation::Sync {
            prune: true,
            remote: Some("origin".to_owned()),
        },
        &["stack", "sync", "--prune", "--remote", "origin"],
    );
    assert_arguments(&StackOperation::Bottom, &["stack", "bottom"]);
    assert_arguments(&StackOperation::Down(2), &["stack", "down", "2"]);
    assert_arguments(&StackOperation::Top, &["stack", "top"]);
    assert_arguments(&StackOperation::Trunk, &["stack", "trunk"]);
    assert_arguments(&StackOperation::Up(3), &["stack", "up", "3"]);
}

#[test]
fn stack_previews_name_destructive_scope_and_merge_method() {
    assert_eq!(
        StackOperation::Add {
            branch: "tests".to_owned(),
            all: true,
            update: false,
            message: Some("Add tests".to_owned()),
        }
        .preview_message(),
        "Would add branch tests, stage all changes, and commit them as `Add tests`. Pass --yes to continue."
    );
    assert_eq!(
        StackOperation::Add {
            branch: "fix".to_owned(),
            all: false,
            update: true,
            message: Some("Fix bug".to_owned()),
        }
        .preview_message(),
        "Would add branch fix, stage tracked changes, and commit them as `Fix bug`. Pass --yes to continue."
    );
    assert_eq!(
        StackOperation::Add {
            branch: "docs".to_owned(),
            all: false,
            update: false,
            message: Some("Write docs".to_owned()),
        }
        .preview_message(),
        "Would add branch docs and commit staged changes as `Write docs`. Pass --yes to continue."
    );
    assert_eq!(
        StackOperation::Unstack {
            stack: Some("7".to_owned()),
            local: false,
        }
        .preview_message(),
        "Would remove local and GitHub tracking for stack 7. Pass --yes to continue."
    );
    assert_eq!(
        StackOperation::Sync {
            prune: true,
            remote: None,
        }
        .preview_message(),
        "Would synchronize the active stack and prune merged local branches. Pass --yes to continue."
    );
    assert_eq!(
        StackOperation::Merge {
            target: Some("42".to_owned()),
            method: PullRequestMergeMethod::Squash,
        }
        .preview_message(),
        "Would atomically squash and merge stack 42. Pass --yes to continue."
    );
}
