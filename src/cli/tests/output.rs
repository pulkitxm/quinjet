use super::*;

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
