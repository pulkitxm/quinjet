use super::*;

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
