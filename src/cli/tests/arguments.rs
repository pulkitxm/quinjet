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
fn the_global_folder_alias_selects_an_implicit_or_explicit_terminal() {
    let implicit = Cli::try_parse_from(["quinjet", "--folder", "/tmp/implicit"]).unwrap();
    assert!(implicit.command.is_none());
    assert_eq!(implicit.repository, Path::new("/tmp/implicit"));

    let explicit = Cli::try_parse_from(["quinjet", "tui", "--folder", "/tmp/explicit"]).unwrap();
    assert!(matches!(explicit.command, Some(Verb::Tui(_))));
    assert_eq!(explicit.repository, Path::new("/tmp/explicit"));
}

#[test]
fn edith_client_is_available_to_terminal_launches() {
    let cli = Cli::try_parse_from(["quinjet", "--client", "edith", "tui", "/tmp/project"]).unwrap();
    assert_eq!(cli.client, Some(Client::Edith));
    drop(Cli::try_parse_from(["quinjet", "--client", "unknown"]).unwrap_err());
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
fn pull_request_collaboration_actions_parse_without_prompts() {
    let review = Cli::try_parse_from([
        "quinjet",
        "pr",
        "review",
        "24",
        "--request-changes",
        "--body",
        "Needs tests",
        "--yes",
    ])
    .unwrap();
    assert!(matches!(
        review.command,
        Some(Verb::Pr {
            command: PrVerb::Review(PrReviewArgs {
                choice: PrReviewChoiceArgs {
                    request_changes: true,
                    ..
                },
                yes: true,
                ..
            })
        })
    ));

    let edit = Cli::try_parse_from([
        "quinjet",
        "pr",
        "edit",
        "24",
        "add-reviewer",
        "octocat,hubot",
        "--yes",
    ])
    .unwrap();
    assert!(matches!(
        edit.command,
        Some(Verb::Pr {
            command: PrVerb::Edit(PrEditArgs {
                field: PrEditFieldArg::AddReviewer,
                value: Some(value),
                yes: true,
                ..
            })
        }) if value == "octocat,hubot"
    ));

    let lock =
        Cli::try_parse_from(["quinjet", "pr", "lock", "24", "--reason", "too-heated"]).unwrap();
    assert!(matches!(
        lock.command,
        Some(Verb::Pr {
            command: PrVerb::Lock(PrLockArgs {
                reason: Some(PrLockReasonArg::TooHeated),
                ..
            })
        })
    ));
}

#[test]
fn pull_request_review_verbs_parse_at_their_leaf() {
    let comment = Cli::try_parse_from([
        "quinjet",
        "pr",
        "reviews",
        "comment",
        "24",
        "src/main.rs",
        "--line",
        "42",
        "--side",
        "right",
        "--body",
        "Fix this",
    ])
    .unwrap();
    assert!(matches!(
        comment.command,
        Some(Verb::Pr {
            command: PrVerb::Reviews {
                command: PrReviewVerb::Comment(PrReviewCommentArgs {
                    line: Some(42),
                    side: Some(PrReviewSideArg::Right),
                    ..
                })
            }
        })
    ));

    let submit = Cli::try_parse_from([
        "quinjet",
        "pr",
        "reviews",
        "submit",
        "24",
        "--approve",
        "--body-file",
        "review.md",
    ])
    .unwrap();
    assert!(matches!(
        submit.command,
        Some(Verb::Pr {
            command: PrVerb::Reviews {
                command: PrReviewVerb::Submit(PrReviewSubmitArgs {
                    decision: PrReviewDecisionArgs { approve: true, .. },
                    ..
                })
            }
        })
    ));

    Cli::try_parse_from([
        "quinjet",
        "pr",
        "reviews",
        "comment",
        "24",
        "src/main.rs",
        "--body",
        "Missing coordinates",
    ])
    .unwrap_err();
}

fn assert_argument_cases(cases: &str, succeeds: bool) -> usize {
    let mut count = 0;
    for arguments in cases
        .split(['|', '\n'])
        .map(str::trim)
        .filter(|arguments| !arguments.is_empty())
    {
        let parsed = Cli::try_parse_from(
            std::iter::once("quinjet").chain(arguments.split_ascii_whitespace()),
        )
        .is_ok();
        if succeeds {
            assert!(parsed, "expected valid arguments: {arguments}");
        } else {
            assert!(!parsed, "expected invalid arguments: {arguments}");
        }
        count += 1;
    }
    count
}

#[test]
fn argument_relationships_accept_the_supported_combinations() {
    let cases = "
-C /tmp --json diff --staged src|branch list --json -C /tmp|pr reviews show 7 -C /tmp --json
stage src|stage --all|unstage src|unstage --all|discard --all --yes
rm generated --yes|status --watch --interval 1|diff --unstaged --expanded src
resolve file --ours|resolve file --theirs|resolve file --stage
stash push --staged -m saved|stash push --include-untracked src|stash pop stash@{0}
completions --install|completions fish --install|completion zsh|log HEAD --skip 2 --limit 4
branch create topic HEAD|pr view 7 --watch --interval 2|pr conversation 7 --watch --interval 2
pr checks 7 --watch --interval 2|pr checks 7 --exit-code|pr logs 7 lint --watch --interval 3
pr admin-merge 7 --merge|pr auto-merge 7 --rebase --delete-branch|pr merge 7 --merge
pr review 7 --approve|pr review 7 --comment --body note|pr update-branch 7 --rebase --yes
pr revert 7 --title undo --body reason --draft --yes|pr lock 7 --reason off-topic --yes
pr edit 7 title value|pr edit 7 body value|pr edit 7 base main
pr edit 7 add-assignee octo|pr edit 7 remove-assignee octo|pr edit 7 add-label bug
pr edit 7 remove-label bug|pr edit 7 add-project road|pr edit 7 remove-project road
pr edit 7 add-reviewer octo|pr edit 7 remove-reviewer octo|pr edit 7 milestone v1
pr edit 7 remove-milestone
pr reviews comment 7 file --line 2 --side left --start-line 1 --start-side left -b note
pr reviews comment 7 file --file --body-file note.txt|pr reviews reply 7 thread -b reply
pr reviews reply 7 thread --body-file -|pr reviews edit 7 comment --body-file note.txt
pr reviews delete 7 comment --yes|pr reviews submit 7 --comment -b note
pr reviews submit 7 --request-changes --body-file note.txt|pr reviews discard 7 --yes
pr reviews resolve 7 thread|pr reviews unresolve 7 thread
";
    assert_eq!(assert_argument_cases(cases, true), 59);
}

#[test]
fn argument_relationships_reject_unsupported_combinations() {
    let cases = "
stage|stage src --all|unstage|unstage src --all|discard|discard src --all
remove|remove src --all|status --interval 1|status --watch --interval 0
diff --staged --unstaged|resolve file|resolve file --ours --theirs
resolve file --ours --stage|resolve file --theirs --stage|stash push --staged --include-untracked
completions|completions bash --automatic|completions --automatic|commit -m|branch create
pr view 7 --interval 2|pr view 7 --watch --interval 1
pr conversation 7 --interval 2|pr conversation 7 --watch --interval 1
pr checks 7 --interval 2|pr checks 7 --watch --interval 1|pr checks 7 --watch --exit-code
pr logs 7 lint --interval 3|pr logs 7 lint --watch --interval 2
pr admin-merge 7|pr admin-merge 7 --merge --squash
pr auto-merge 7|pr auto-merge 7 --rebase --merge|pr merge 7 --squash --rebase
pr review 7|pr review 7 --approve --comment|pr review 7 --comment --request-changes|pr review 7 --approve --request-changes
pr edit 7 unknown value|pr lock 7 --reason noisy
pr reviews comment 7 file --line 2 -b note
pr reviews comment 7 file --side right -b note
pr reviews comment 7 file --line 2 --side right --start-side left -b note
pr reviews comment 7 file --file --line 2 --side right -b note
pr reviews comment 7 file --file -b note --body-file note.txt
pr reviews comment 7 file --line 2 --side right
pr reviews reply 7 thread|pr reviews reply 7 thread -b note --body-file note.txt
pr reviews edit 7 comment|pr reviews edit 7 comment -b note --body-file note.txt
pr reviews submit 7 -b note|pr reviews submit 7 --approve --comment -b note
pr reviews submit 7 --approve|pr reviews submit 7 --approve -b note --body-file note.txt
pr reviews delete 7|pr reviews resolve 7|pr reviews unresolve 7
";
    assert_eq!(assert_argument_cases(cases, false), 58);
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
