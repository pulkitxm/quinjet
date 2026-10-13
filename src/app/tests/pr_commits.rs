use super::*;
use crate::git::github::{PullRequestCommit, PullRequestCommits};

fn commit_app() -> App {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(42, "Ship rocket", "acme/widget"));
    app.pr_commits.loaded = true;
    app.pr_commits.snapshot.commits = vec![
        PullRequestCommit {
            oid: "a".repeat(40),
            abbreviated_oid: "aaaaaaa".to_owned(),
            subject: "Add launch".to_owned(),
            ..PullRequestCommit::default()
        },
        PullRequestCommit {
            oid: "b".repeat(40),
            abbreviated_oid: "bbbbbbb".to_owned(),
            subject: "Add engines".to_owned(),
            ..PullRequestCommit::default()
        },
    ];
    app
}

#[test]
fn choosing_a_commit_scopes_files_and_checks_and_rejects_old_responses() {
    let mut app = commit_app();
    app.pull_request_checks = vec![check("head checks", PullRequestCheckStatus::Passed)];
    let stale_checks = app.pull_request_checks_generation;
    let stale_diff = app.diff_generation;
    let mut effects = Vec::new();
    app.inspect_pr_commit(Some(1), &mut effects);
    assert_eq!(app.pull_request_section, PullRequestSection::Files);
    assert_eq!(app.selected_pr_commit().unwrap().oid, "b".repeat(40));
    assert_eq!(app.pull_request_checks, Vec::new());
    assert!(effects.iter().any(|effect| matches!(effect, AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::PreparePullRequest { commit: Some(oid), .. } if oid == &"b".repeat(40)))));
    assert!(effects.iter().any(|effect| matches!(effect, AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::LoadPullRequestChecks { commit: Some(oid), .. } if oid == &"b".repeat(40)))));
    drop(app.handle_worker_event(
        WorkerEvent::PullRequestChecks {
            generation: stale_checks,
            result: Ok(crate::git::github::PullRequestChecks {
                checks: vec![check("wrong head", PullRequestCheckStatus::Passed)],
                from_cache: false,
            }),
        },
        Instant::now(),
    ));
    drop(app.handle_worker_event(
        WorkerEvent::PullRequestIndex {
            generation: stale_diff,
            result: Ok(PullRequestDiffIndex {
                files: Vec::new(),
                total_files: 0,
                truncated: false,
            }),
        },
        Instant::now(),
    ));
    assert_eq!(app.pull_request_checks, Vec::new());
    assert_eq!(app.pull_request_files, Vec::new());
}

#[test]
fn switching_commits_invalidates_logs_and_returning_restores_pr_scope() {
    let mut app = commit_app();
    let mut effects = Vec::new();
    app.inspect_pr_commit(Some(0), &mut effects);
    let old_log = app.pull_request_check_log_generation;
    app.inspect_pr_commit(Some(1), &mut effects);
    assert!(app.pull_request_check_log_generation > old_log);
    app.inspect_pr_commit(None, &mut effects);
    assert!(app.selected_pr_commit().is_none());
    assert!(
        matches!(effects.last(), Some(AppEffect::Git(command)) if matches!(command.as_ref(), WorkerCommand::LoadPullRequestChecks { commit: None, .. }))
    );
    assert_eq!(
        app.pull_request_diff_source,
        Some(PullRequestDiffSource::PullRequest)
    );
}

#[test]
fn commit_list_ignores_stale_generations_and_orders_newest_first() {
    let mut app = commit_app();
    app.pr_commits.loaded = false;
    let mut effects = Vec::new();
    app.request_pr_commits(&mut effects);
    let generation = app.pr_commits.generation;
    let request = app.pull_request.as_ref().unwrap();
    let snapshot = PullRequestCommits {
        commits: app.pr_commits.snapshot.commits.clone(),
        total_commits: 2,
        base_oid: request.base_oid.clone(),
        head_oid: request.head_oid.clone(),
        ..PullRequestCommits::default()
    };
    app.apply_pr_commits(
        generation.wrapping_sub(1),
        Ok(snapshot.clone()),
        &mut effects,
    );
    assert!(!app.pr_commits.loaded);
    app.apply_pr_commits(generation, Ok(snapshot), &mut effects);
    assert!(app.pr_commits.loaded);
    assert_eq!(app.pr_commits.snapshot.commits[0].oid, "b".repeat(40));
}

#[test]
fn keyboard_commit_inspection_has_files_ci_and_a_return_route() {
    let mut app = commit_app();
    let now = Instant::now();
    drop(app.handle_key(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT), now));
    assert_eq!(app.pull_request_section, PullRequestSection::Commits);
    drop(app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now));
    drop(app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now));
    assert_eq!(app.selected_pr_commit().unwrap().oid, "b".repeat(40));
    drop(app.handle_key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT), now));
    assert_eq!(app.pull_request_section, PullRequestSection::Overview);
    drop(app.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT), now));
    assert!(app.selected_pr_commit().is_none());
}

#[test]
fn stack_commit_selection_uses_its_member_instead_of_the_root_pr() {
    let mut app = commit_app();
    app.pull_request_stack = Some(pull_request_stack(2));
    app.pull_request_stack_cursor = Some(1);
    app.pull_request_section = PullRequestSection::Stack;
    app.reconcile_stack_inspector();
    app.stack_inspector.commits = app.pr_commits.snapshot.clone();
    let mut effects = Vec::new();
    app.inspect_stack_commit(1, &mut effects);
    assert_eq!(app.selected_pull_request().unwrap().number, 41);
    assert_eq!(app.selected_pr_commit().unwrap().oid, "b".repeat(40));
    assert!(effects.iter().any(|effect| matches!(effect, AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::PreparePullRequest { pull_request, commit: Some(_), .. } if pull_request.number == 41))));
    app.select_pull_request_section(PullRequestSection::Stack, &mut effects);
    assert!(app.selected_pr_commit().is_none());
    assert!(app.pr_commits.owner.is_none());
}

#[test]
fn changed_pr_boundaries_reload_the_visible_commit_list() {
    for change_base in [false, true] {
        let mut app = commit_app();
        app.pull_request_section = PullRequestSection::Commits;
        let mut current = app.pull_request.clone().unwrap();
        if change_base {
            current.base_oid = "new-base".to_owned();
        } else {
            current.head_oid = "new-head".to_owned();
        }
        let old_generation = app.pr_commits.generation;
        let effects = app.handle_worker_event(
            WorkerEvent::PullRequestLookup {
                generation: app.pull_request_generation,
                result: Ok(crate::git::github::PullRequestSnapshot {
                    repositories: Vec::new(),
                    selected_repository: None,
                    pull_request: current.clone(),
                    warnings: Vec::new(),
                    exact_number: Some(current.number),
                    from_cache: false,
                }),
            },
            Instant::now(),
        );
        assert!(!app.pr_commits.loaded);
        assert!(app.pr_commits.generation > old_generation);
        assert!(effects.iter().any(|effect| matches!(effect, AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::LoadPullRequestCommits { pull_request, .. } if pull_request.base_oid == current.base_oid && pull_request.head_oid == current.head_oid))));
    }
}

#[test]
fn refreshed_stack_boundaries_replace_the_commit_owner_and_reject_old_history() {
    let mut app = commit_app();
    app.pull_request_stack = Some(pull_request_stack(2));
    app.pull_request_stack_cursor = Some(1);
    app.pull_request_section = PullRequestSection::Stack;
    app.reconcile_stack_inspector();
    app.stack_inspector.commits = app.pr_commits.snapshot.clone();
    let mut effects = Vec::new();
    app.inspect_stack_commit(1, &mut effects);
    app.pull_request_section = PullRequestSection::Commits;
    let stale_generation = app.pr_commits.generation;
    let stale_snapshot = app.pr_commits.snapshot.clone();
    let mut stack = app.pull_request_stack.clone().unwrap();
    stack.members[0].head_oid = "new-member-head".to_owned();
    effects.clear();
    app.apply_pull_request_stack_snapshot(Some(stack), &mut effects);
    assert_eq!(app.selected_pull_request().unwrap().number, 41);
    assert_eq!(
        app.selected_pull_request().unwrap().head_oid,
        "new-member-head"
    );
    assert!(!app.pr_commits.loaded);
    assert!(app.selected_pr_commit().is_none());
    assert!(effects.iter().any(|effect| matches!(effect, AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::LoadPullRequestCommits { pull_request, .. } if pull_request.number == 41 && pull_request.head_oid == "new-member-head"))));
    app.apply_pr_commits(stale_generation, Ok(stale_snapshot), &mut effects);
    assert!(!app.pr_commits.loaded);
}

#[test]
fn running_commit_checks_in_a_stack_keep_the_fast_poll_interval() {
    let mut app = commit_app();
    app.pull_request_stack = Some(pull_request_stack(2));
    app.pr_commits.owner = app.pull_request.clone();
    app.pull_request_checks = vec![check("commit build", PullRequestCheckStatus::Pending)];
    assert_eq!(app.pull_request_poll_interval(), PULL_REQUEST_ACTIVE_POLL);
    app.pull_request_checks[0].status = PullRequestCheckStatus::Passed;
    assert_eq!(app.pull_request_poll_interval(), PULL_REQUEST_IDLE_POLL);
    app.pr_commits.loading = true;
    assert!(app.pull_request_refreshing());
}

#[test]
fn switching_from_stack_commits_loads_full_history_without_changing_the_selected_hash() {
    let mut app = commit_app();
    app.pr_commits.introduced_only = true;
    app.pr_commits.selected = Some(1);
    let selected = app.selected_pr_commit().unwrap().oid.clone();
    let mut effects = Vec::new();
    app.request_pr_commits(&mut effects);
    assert!(app.pr_commits.loading);
    assert!(effects.iter().any(|effect| matches!(effect, AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::LoadPullRequestCommits { .. }))));
    let request = app.pull_request.as_ref().unwrap();
    let mut commits = app.pr_commits.snapshot.commits.clone();
    commits.insert(
        0,
        PullRequestCommit {
            oid: "c".repeat(40),
            ..PullRequestCommit::default()
        },
    );
    let snapshot = PullRequestCommits {
        commits,
        total_commits: 3,
        base_oid: request.base_oid.clone(),
        head_oid: request.head_oid.clone(),
        ..PullRequestCommits::default()
    };
    app.apply_pr_commits(app.pr_commits.generation, Ok(snapshot), &mut effects);
    assert!(!app.pr_commits.introduced_only);
    assert_eq!(app.selected_pr_commit().unwrap().oid, selected);
}
