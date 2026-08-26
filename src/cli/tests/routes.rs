use super::*;
use crate::git::{StackModifyAction, StackOperation, StackRebaseAction};

fn resolves(path: &[&str]) -> bool {
    let root = Cli::command();
    let mut command = &root;
    let mut found = Vec::new();
    for name in path {
        let Some(child) = command.find_subcommand(name) else {
            return false;
        };
        found.push(child);
        command = found.last().unwrap();
    }
    true
}

macro_rules! operation_routes {
        ($($pattern:pat => $sample:expr => [$($path:literal),+];)+) => {
            const fn verb_for(operation: &GitOperation) -> &'static [&'static str] {
                match operation {
                    $($pattern => &[$($path),+],)+
                }
            }

            #[test]
            fn every_operation_the_interface_performs_has_a_verb() {
                let operations = [$($sample),+];
                for operation in &operations {
                    let path = verb_for(operation);
                    assert!(
                        resolves(path),
                        "{operation:?} names the verb {path:?}, which the command tree does not have"
                    );
                }
                let kinds: HashSet<_> = operations.iter().map(std::mem::discriminant).collect();
                assert_eq!(
                    kinds.len(),
                    operations.len(),
                    "every operation variant must have one route fixture"
                );
            }
        };
    }

operation_routes! {
    GitOperation::Stage(_) => GitOperation::Stage(Vec::new()) => ["stage"];
    GitOperation::StageAll => GitOperation::StageAll => ["stage"];
    GitOperation::Unstage(_) => GitOperation::Unstage(Vec::new()) => ["unstage"];
    GitOperation::UnstageAll => GitOperation::UnstageAll => ["unstage"];
    GitOperation::Discard(_) => GitOperation::Discard(Vec::new()) => ["discard"];
    GitOperation::Remove(_) => GitOperation::Remove(Vec::new()) => ["remove"];
    GitOperation::Commit { .. } => GitOperation::Commit { message: String::new(), amend: false } => ["commit"];
    GitOperation::Fetch => GitOperation::Fetch => ["fetch"];
    GitOperation::Pull => GitOperation::Pull => ["pull"];
    GitOperation::Push => GitOperation::Push => ["push"];
    GitOperation::Sync => GitOperation::Sync => ["sync"];
    GitOperation::Checkout(_) => GitOperation::Checkout(String::new()) => ["branch", "switch"];
    GitOperation::CreateBranch { .. } => GitOperation::CreateBranch { name: String::new(), start: None } => ["branch", "create"];
    GitOperation::RenameBranch { .. } => GitOperation::RenameBranch { old: String::new(), new: String::new() } => ["branch", "rename"];
    GitOperation::DeleteBranch(_) => GitOperation::DeleteBranch(String::new()) => ["branch", "delete"];
    GitOperation::StashPush { .. } => GitOperation::StashPush { message: String::new(), include_untracked: false, staged: false, paths: Vec::new() } => ["stash", "push"];
    GitOperation::StashApply(_) => GitOperation::StashApply(String::new()) => ["stash", "apply"];
    GitOperation::StashPop(_) => GitOperation::StashPop(None) => ["stash", "pop"];
    GitOperation::StashDrop(_) => GitOperation::StashDrop(String::new()) => ["stash", "drop"];
    GitOperation::StashClear => GitOperation::StashClear => ["stash", "clear"];
    GitOperation::ResolveConflict { .. } => GitOperation::ResolveConflict { path: PathBuf::new(), choice: ConflictChoice::Ours } => ["resolve"];
    GitOperation::CherryPick(_) => GitOperation::CherryPick(String::new()) => ["cherry-pick"];
    GitOperation::Revert(_) => GitOperation::Revert(String::new()) => ["revert"];
    GitOperation::Stack(_) => GitOperation::Stack(Box::new(StackOperation::Top)) => ["stack", "top"];
}

const fn stack_verb_for(operation: &StackOperation) -> &'static [&'static str] {
    match operation {
        StackOperation::Init { .. } => &["stack", "init"],
        StackOperation::Add { .. } => &["stack", "add"],
        StackOperation::Checkout(_) => &["stack", "checkout"],
        StackOperation::Modify(_) => &["stack", "modify"],
        StackOperation::Unstack { .. } => &["stack", "unstack"],
        StackOperation::Link { .. } => &["stack", "link"],
        StackOperation::Merge { .. } => &["stack", "merge"],
        StackOperation::Push { .. } => &["stack", "push"],
        StackOperation::Rebase(_) => &["stack", "rebase"],
        StackOperation::Submit { .. } => &["stack", "submit"],
        StackOperation::Sync { .. } => &["stack", "sync"],
        StackOperation::Bottom => &["stack", "bottom"],
        StackOperation::Down(_) => &["stack", "down"],
        StackOperation::Top => &["stack", "top"],
        StackOperation::Trunk => &["stack", "trunk"],
        StackOperation::Up(_) => &["stack", "up"],
    }
}

#[test]
fn every_stack_operation_has_a_verb() {
    let operations = [
        StackOperation::Init {
            branches: Vec::new(),
            base: None,
        },
        StackOperation::Add {
            branch: String::new(),
            all: false,
            update: false,
            message: None,
        },
        StackOperation::Checkout(String::new()),
        StackOperation::Modify(StackModifyAction::Abort),
        StackOperation::Unstack {
            stack: None,
            local: false,
        },
        StackOperation::Link {
            members: Vec::new(),
            base: None,
            open: false,
            remote: None,
        },
        StackOperation::Merge {
            target: None,
            method: PullRequestMergeMethod::Squash,
        },
        StackOperation::Push { remote: None },
        StackOperation::Rebase(StackRebaseAction::Abort),
        StackOperation::Submit {
            open: false,
            remote: None,
        },
        StackOperation::Sync {
            prune: false,
            remote: None,
        },
        StackOperation::Bottom,
        StackOperation::Down(1),
        StackOperation::Top,
        StackOperation::Trunk,
        StackOperation::Up(1),
    ];
    for operation in &operations {
        let path = stack_verb_for(operation);
        assert!(resolves(path), "stack operation is missing {path:?}");
    }
    let kinds: HashSet<_> = operations.iter().map(std::mem::discriminant).collect();
    assert_eq!(kinds.len(), operations.len());
}

#[test]
fn the_read_only_views_have_verbs_too() {
    for path in [
        ["status"].as_slice(),
        ["diff"].as_slice(),
        ["log"].as_slice(),
        ["show"].as_slice(),
        ["branch", "list"].as_slice(),
        ["branch", "compare"].as_slice(),
        ["stash", "list"].as_slice(),
        ["stash", "show"].as_slice(),
        ["worktree", "list"].as_slice(),
        ["project", "list"].as_slice(),
        ["repos"].as_slice(),
        ["pr", "view"].as_slice(),
        ["pr", "files"].as_slice(),
        ["pr", "diff"].as_slice(),
        ["pr", "checks"].as_slice(),
        ["pr", "conversation"].as_slice(),
        ["pr", "logs"].as_slice(),
        ["pr", "open"].as_slice(),
        ["pr", "reviews", "show"].as_slice(),
        ["stack", "view"].as_slice(),
        ["stack", "files"].as_slice(),
        ["stack", "diff"].as_slice(),
        ["tui"].as_slice(),
        ["completions"].as_slice(),
        ["man"].as_slice(),
        ["capabilities"].as_slice(),
    ] {
        assert!(resolves(path), "the command tree is missing {path:?}");
    }
}
