use std::collections::HashSet;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use indicatif::InMemoryTerm;

use super::*;
use crate::git::github::PullRequestCheck;

static TEST_REPOSITORY_ID: AtomicUsize = AtomicUsize::new(0);

struct TestRepository {
    path: PathBuf,
}

impl TestRepository {
    fn new() -> Self {
        let id = TEST_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
        let name = format!("quinjet-cli-test-{}-{id}", std::process::id());
        // nosemgrep: rust.lang.security.temp-dir.temp-dir
        let path = std::env::temp_dir().join(name);
        drop(fs::remove_dir_all(&path));
        fs::create_dir_all(&path).unwrap();
        let repository = Self { path };
        repository.git(&["init", "--initial-branch=main"]);
        repository.git(&["config", "user.name", "Quinjet Test"]);
        repository.git(&["config", "user.email", "quinjet@example.com"]);
        fs::write(repository.path.join("README.md"), "one\n").unwrap();
        repository.git(&["add", "README.md"]);
        repository.git(&["commit", "--message=base"]);
        repository
    }

    fn git(&self, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn session(&self) -> Session {
        Session::new(Repository::discover(&self.path).unwrap())
    }
}

impl Drop for TestRepository {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

fn check(name: &str, status: PullRequestCheckStatus) -> PullRequestCheck {
    PullRequestCheck {
        name: name.to_owned(),
        workflow: "CI".to_owned(),
        state: String::new(),
        status,
        description: String::new(),
        link: String::new(),
        started_at: String::new(),
        completed_at: String::new(),
    }
}

#[test]
fn the_command_tree_is_unambiguous() {
    Cli::command().debug_assert();
}

#[test]
fn progress_requires_human_output_and_an_interactive_stderr() {
    assert!(progress_enabled(false, true));
    assert!(!progress_enabled(true, true));
    assert!(!progress_enabled(false, false));
}

#[test]
fn progress_updates_its_phase_and_clears_when_finished() {
    let terminal = InMemoryTerm::new(2, 80);
    let progress = progress_bar(
        "Loading pull request",
        ProgressDrawTarget::term_like(Box::new(terminal.clone())),
    )
    .unwrap();
    progress.tick();
    assert!(terminal.contents().contains("Loading pull request"));
    let out = Emitter {
        json: false,
        progress: Some(progress),
    };

    out.set_progress("Fetching pull-request checks");
    out.progress.as_ref().unwrap().tick();
    assert!(terminal.contents().contains("Fetching pull-request checks"));
    out.note("warning: using stale metadata");
    assert!(
        terminal
            .contents()
            .contains("warning: using stale metadata")
    );

    out.finish_progress();
    assert_eq!(terminal.contents(), "warning: using stale metadata");
}

#[test]
fn completion_hints_distinguish_paths_from_identifiers() {
    let root = Cli::command();
    let diff = root.find_subcommand("diff").unwrap();
    let diff_path = diff
        .get_arguments()
        .find(|argument| argument.get_id() == "paths")
        .unwrap();
    assert_eq!(diff_path.get_value_hint(), ValueHint::AnyPath);

    let branch = root.find_subcommand("branch").unwrap();
    let switch = branch.find_subcommand("switch").unwrap();
    let branch_name = switch
        .get_arguments()
        .find(|argument| argument.get_id() == "name")
        .unwrap();
    assert_eq!(branch_name.get_value_hint(), ValueHint::Other);

    let status = root.find_subcommand("status").unwrap();
    let interval = status
        .get_arguments()
        .find(|argument| argument.get_id() == "interval")
        .unwrap();
    assert_eq!(interval.get_value_hint(), ValueHint::Other);
}

#[test]
fn a_terminal_path_belongs_to_the_tui_verb() {
    let cli = Cli::try_parse_from(["quinjet", "tui", "/tmp/somewhere"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Verb::Tui(TuiArgs { path, .. })) if path == Path::new("/tmp/somewhere")
    ));
}

#[test]
fn terminal_themes_default_to_quinjet_with_system_appearance() {
    let cli = Cli::try_parse_from(["quinjet", "tui"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Verb::Tui(TuiArgs {
            theme: ThemeName::Quinjet,
            appearance: AppearanceChoice::System,
            ..
        }))
    ));

    let cli = Cli::try_parse_from([
        "quinjet",
        "tui",
        "--theme",
        "rose-pine",
        "--appearance",
        "light",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Some(Verb::Tui(TuiArgs {
            theme: ThemeName::RosePine,
            appearance: AppearanceChoice::Light,
            ..
        }))
    ));
    drop(Cli::try_parse_from(["quinjet", "tui", "--theme", "unknown"]).unwrap_err());
    drop(Cli::try_parse_from(["quinjet", "tui", "--appearance", "unknown"]).unwrap_err());
}

#[test]
fn an_unknown_verb_is_a_usage_error() {
    drop(Cli::try_parse_from(["quinjet", "statsu"]).unwrap_err());
}

#[test]
fn a_verb_is_never_mistaken_for_a_repository_path() {
    let cli = Cli::try_parse_from(["quinjet", "status"]).unwrap();
    assert!(
        matches!(cli.command, Some(Verb::Status(_))),
        "`quinjet status` must reach the status verb rather than open a directory called status"
    );
}

#[test]
fn every_subcommand_answers_to_the_repository_and_json_switches() {
    for argv in [
        vec!["quinjet", "-C", "/tmp/elsewhere", "--json", "status"],
        vec!["quinjet", "status", "-C", "/tmp/elsewhere", "--json"],
        vec![
            "quinjet",
            "pr",
            "checks",
            "1",
            "--json",
            "-C",
            "/tmp/elsewhere",
        ],
    ] {
        let cli = Cli::try_parse_from(&argv).unwrap();
        assert!(cli.json, "{argv:?} must be readable as JSON");
        assert_eq!(cli.repository, PathBuf::from("/tmp/elsewhere"), "{argv:?}");
    }
}

#[test]
fn pull_request_live_and_browser_options_parse_at_their_leaf() {
    let view =
        Cli::try_parse_from(["quinjet", "pr", "view", "24", "--watch", "--interval", "9"]).unwrap();
    assert!(matches!(
        view.command,
        Some(Verb::Pr {
            command: PrVerb::View(PrWatchArgs {
                watch: true,
                interval: 9,
                ..
            })
        })
    ));

    let open = Cli::try_parse_from(["quinjet", "pr", "open", "24", "--check", "Clippy"]).unwrap();
    assert!(matches!(
        open.command,
        Some(Verb::Pr {
            command: PrVerb::Open(PrOpenArgs {
                check: Some(name),
                ..
            })
        }) if name == "Clippy"
    ));

    let merge = Cli::try_parse_from([
        "quinjet",
        "pr",
        "merge",
        "24",
        "--squash",
        "--delete-branch",
        "--yes",
    ])
    .unwrap();
    assert!(matches!(
        merge.command,
        Some(Verb::Pr {
            command: PrVerb::Merge(PrMergeArgs {
                method: PrMergeMethodArgs {
                    squash: true,
                    merge: false,
                    rebase: false,
                },
                delete_branch: true,
                yes: true,
                ..
            })
        })
    ));

    let close = Cli::try_parse_from(["quinjet", "pr", "close", "24"]).unwrap();
    assert!(matches!(
        close.command,
        Some(Verb::Pr {
            command: PrVerb::Close(PrMutateArgs { yes: false, .. })
        })
    ));

    assert!(
        Cli::try_parse_from(["quinjet", "pr", "merge", "24"]).is_err(),
        "merge without a method must be rejected"
    );
}

#[test]
fn a_session_answers_a_status_command_with_the_working_tree() {
    let repository = TestRepository::new();
    fs::write(repository.path.join("added.txt"), "new\n").unwrap();
    let mut session = repository.session();

    let status = session.execute(Command::Status).unwrap().status().unwrap();

    assert_eq!(status.branch.head, "main");
    assert!(
        status
            .changes
            .iter()
            .any(|change| change.path == Path::new("added.txt")),
        "the untracked file belongs in the answer: {status:?}"
    );
}

#[test]
fn a_prepared_workspace_answers_only_the_generation_that_asked_for_it() {
    let repository = TestRepository::new();
    fs::write(repository.path.join("README.md"), "two\n").unwrap();
    let mut session = repository.session();
    let status = session.execute(Command::Status).unwrap().status().unwrap();
    session
        .execute(Command::PrepareLocalDiff {
            workspace: 7,
            request: Box::new(LocalDiffRequest::Changes {
                changes: status.changes,
                version: 0,
                expanded: false,
            }),
        })
        .unwrap();

    let mine = session.execute(Command::LocalDiffFile {
        workspace: 7,
        path: PathBuf::from("README.md"),
    });
    let stale = session.execute(Command::LocalDiffFile {
        workspace: 8,
        path: PathBuf::from("README.md"),
    });

    assert!(
        mine.is_ok(),
        "the generation that prepared it must be answered"
    );
    assert!(
        stale.is_err(),
        "a workspace must never answer a generation it was not prepared for"
    );
}

#[test]
fn an_operation_command_reports_what_it_did() {
    let repository = TestRepository::new();
    fs::write(repository.path.join("added.txt"), "new\n").unwrap();
    let mut session = repository.session();

    let (label, changes_history, message) = session
        .execute(Command::Operate(GitOperation::StageAll))
        .unwrap()
        .operation()
        .unwrap();

    assert_eq!(label, "Staging all changes");
    assert!(!changes_history);
    assert_eq!(message, "All changes staged");
    assert!(
        repository
            .git(&["diff", "--cached", "--name-only"])
            .contains("added.txt"),
        "staging through the command layer must reach the index"
    );
}

#[test]
fn a_revision_resolves_from_what_a_person_would_type() {
    let repository = TestRepository::new();
    let head = repository.git(&["rev-parse", "HEAD"]);
    repository.git(&["tag", "v1"]);
    let session = repository.session();

    assert_eq!(session.repository_revision("HEAD").unwrap(), "HEAD");
    assert_eq!(
        session.repository_revision("main").unwrap(),
        "refs/heads/main"
    );
    assert_eq!(session.repository_revision("v1").unwrap(), "refs/tags/v1");
    let short: String = head.chars().take(8).collect();
    assert_eq!(session.repository_revision(&short).unwrap(), head);
}

#[test]
fn a_revision_that_names_nothing_is_a_name_that_was_not_found() {
    let repository = TestRepository::new();
    let session = repository.session();

    let error = revision(&session, "deadbeefdead").unwrap_err();
    let failure = error.downcast_ref::<Failure>().unwrap();

    assert_eq!(
        failure.code, EXIT_NOT_FOUND,
        "a revision that resolves to nothing is a missing name, not a failed command"
    );
    assert!(
        failure.hint.is_some(),
        "exit 3 always says what could be named instead"
    );
}

#[test]
fn a_revision_that_could_be_read_as_an_option_is_refused_before_git_sees_it() {
    let repository = TestRepository::new();
    let session = repository.session();

    for revision in ["--output=/tmp/owned", "-n", ""] {
        assert!(
            session.repository_revision(revision).is_err(),
            "`{revision}` must never reach Git as a revision"
        );
    }
}

#[test]
fn watching_checks_stops_only_once_nothing_is_still_running() {
    let running = [
        check("one", PullRequestCheckStatus::Passed),
        check("two", PullRequestCheckStatus::Pending),
    ];
    let settled = [
        check("one", PullRequestCheckStatus::Passed),
        check("two", PullRequestCheckStatus::Failed),
    ];

    assert!(running.iter().any(|check| check.status.is_running()));
    assert!(!settled.iter().any(|check| check.status.is_running()));
    assert_eq!(exit_for(&running), 1, "a pending check is not a green run");
    assert_eq!(exit_for(&settled), 1, "a failed check is not a green run");
    assert_eq!(exit_for(&[check("one", PullRequestCheckStatus::Passed)]), 0);
}

#[test]
fn naming_a_check_that_matches_nothing_says_which_ones_exist() {
    let checks = [
        check(
            "Format, lint, and test (ubuntu-latest)",
            PullRequestCheckStatus::Passed,
        ),
        check(
            "Format, lint, and test (macos-latest)",
            PullRequestCheckStatus::Passed,
        ),
        check("Package validation", PullRequestCheckStatus::Passed),
    ];

    assert_eq!(
        select_check(&checks, "Package validation").unwrap().name,
        "Package validation"
    );
    assert_eq!(
        select_check(&checks, "package").unwrap().name,
        "Package validation",
        "one partial match is enough to name a check"
    );

    let ambiguous = select_check(&checks, "Format").unwrap_err();
    let ambiguous = ambiguous.downcast_ref::<Failure>().unwrap();
    assert_eq!(ambiguous.code, EXIT_NOT_FOUND);
    assert!(ambiguous.hint.as_ref().unwrap().contains("ubuntu-latest"));

    let missing = select_check(&checks, "nothing").unwrap_err();
    assert_eq!(
        missing.downcast_ref::<Failure>().unwrap().code,
        EXIT_NOT_FOUND
    );
}

#[test]
fn unavailable_logs_use_the_same_exit_in_watch_and_one_shot_modes() {
    let log = CheckRunLog {
        unavailable: Some("no Actions job".to_owned()),
        ..CheckRunLog::default()
    };
    let error = ensure_log_available(&log).unwrap_err();
    assert_eq!(
        error.downcast_ref::<Failure>().unwrap().code,
        EXIT_UNAVAILABLE
    );
    ensure_log_available(&CheckRunLog::default()).unwrap();
}

#[test]
fn a_destructive_verb_changes_nothing_until_it_is_confirmed() {
    let repository = TestRepository::new();
    fs::write(repository.path.join("README.md"), "changed\n").unwrap();
    let mut session = repository.session();
    let out = Emitter::new(true);

    discard(
        &mut session,
        &out,
        &DiscardArgs {
            selection: SelectionArgs {
                paths: vec![PathBuf::from("README.md")],
                all: false,
            },
            yes: false,
        },
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(repository.path.join("README.md")).unwrap(),
        "changed\n",
        "a discard without --yes must leave the working tree alone"
    );
}

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
        ["repos"].as_slice(),
        ["pr", "view"].as_slice(),
        ["pr", "files"].as_slice(),
        ["pr", "diff"].as_slice(),
        ["pr", "checks"].as_slice(),
        ["pr", "conversation"].as_slice(),
        ["pr", "logs"].as_slice(),
        ["pr", "open"].as_slice(),
        ["tui"].as_slice(),
        ["completions"].as_slice(),
        ["man"].as_slice(),
        ["capabilities"].as_slice(),
    ] {
        assert!(resolves(path), "the command tree is missing {path:?}");
    }
}
