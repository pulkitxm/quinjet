use super::*;

#[test]
fn explicit_refresh_preserves_the_stack_workspace_and_range() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(42, "Root", "acme/widget"));
    app.pull_request_exact_number = Some(42);
    app.pull_request_stack = Some(pull_request_stack(2));
    app.pull_request_stack_anchor = Some(1);
    app.pull_request_stack_cursor = Some(2);
    app.pull_request_section = PullRequestSection::Stack;
    app.stack_inspector.section = StackMemberSection::Conversation;
    app.stack_inspector.diff_open = true;
    app.document = DiffDocument::empty("Stack range", "keep this diff");
    app.reconcile_stack_inspector();
    let identity = app.stack_inspector.selected_identity.clone();
    let mut effects = Vec::new();

    app.request_active_refresh(&mut effects);

    assert!(app.pull_request.is_some());
    assert!(app.pull_request_stack.is_some());
    assert_eq!(app.pull_request_stack_range(), Some((1, 2)));
    assert_eq!(app.pull_request_section, PullRequestSection::Stack);
    assert_eq!(
        app.stack_inspector.section,
        StackMemberSection::Conversation
    );
    assert_eq!(app.stack_inspector.selected_identity, identity);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LookupPullRequest { refresh: true, .. })
    )));

    let generation = app.pull_request_generation;
    app.handle_worker_event(
        WorkerEvent::PullRequestLookup {
            generation,
            result: Err("refresh unavailable".to_owned()),
        },
        Instant::now(),
    );

    assert_eq!(app.document.title, "Stack range");
    assert!(app.stack_inspector.diff_open);
    assert_eq!(
        app.pull_request_stack_error.as_deref(),
        Some("refresh unavailable")
    );
}

#[test]
fn stack_teardown_invalidates_in_flight_composed_diff_work() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(42, "Root", "acme/widget"));
    app.pull_request_stack = Some(pull_request_stack(2));
    app.pull_request_section = PullRequestSection::Stack;
    app.stack_inspector.diff_open = true;
    app.diff_generation = 7;
    app.document_loading = true;
    app.sidebar_offset = 2;
    app.sidebar_free_scroll = true;
    app.sidebar_last_cursor = Some(1);
    app.pull_request_progress = Some(PullRequestProgress::FindingMergeBase);
    app.pull_request_checks = vec![check("CI", PullRequestCheckStatus::Passed)];
    app.pull_request_conversation.entries = vec![crate::git::github::ConversationEntry {
        kind: crate::git::github::ConversationKind::Comment,
        actor: "octocat".to_owned(),
        timestamp: "2026-08-20T10:00:00Z".to_owned(),
        detail: String::new(),
        body: "old".to_owned(),
        url: String::new(),
        reference: String::new(),
        context: String::new(),
    }];
    app.pull_request_review.head_oid = "old-head".to_owned();
    let mut effects = Vec::new();

    app.apply_pull_request_stack_snapshot(None, &mut effects);

    assert_eq!(app.diff_generation, 8);
    assert!(!app.document_loading);
    assert_eq!(app.pull_request_section, PullRequestSection::Overview);
    assert!(!app.stack_inspector.diff_open);
    assert!(app.pull_request_progress.is_none());
    assert_eq!(app.sidebar_offset, 0);
    assert!(!app.sidebar_free_scroll);
    assert_eq!(app.sidebar_last_cursor, None);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestChecks { refresh: true, .. })
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestConversation { .. })
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestReview { .. })
    )));
}

#[test]
fn changed_stack_topology_invalidates_member_rows() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    let stack = pull_request_stack(2);
    app.sidebar_offset = 2;
    app.sidebar_free_scroll = true;
    app.sidebar_last_cursor = Some(1);
    app.apply_pull_request_stack_snapshot(Some(stack.clone()), &mut Vec::new());
    assert_eq!(app.sidebar_offset, 0);
    assert!(!app.sidebar_free_scroll);
    assert_eq!(app.sidebar_last_cursor, None);
    app.stack_inspector_content_rows_key = Some((StackMemberSection::Summary, 80, 1, 1));
    let mut changed = stack;
    changed.size = 5;
    changed.truncated = true;

    app.apply_pull_request_stack_snapshot(Some(changed), &mut Vec::new());

    assert!(app.stack_inspector_content_rows_key.is_none());
    assert!(app.stack_inspector.tip_identity.is_none());
}

#[test]
fn stack_identity_transition_resets_only_the_pull_request_sidebar() {
    let mut app = App::new("/tmp/repo", "repo");
    app.switch_view(View::PullRequests, &mut Vec::new());
    app.sidebar_offset = 2;
    app.sidebar_free_scroll = true;
    app.sidebar_last_cursor = Some(1);
    app.switch_view(View::History, &mut Vec::new());
    app.sidebar_offset = 7;
    app.sidebar_free_scroll = true;
    app.sidebar_last_cursor = Some(5);

    app.apply_pull_request_stack_snapshot(Some(pull_request_stack(2)), &mut Vec::new());

    assert_eq!(app.sidebar_offset, 7);
    assert!(app.sidebar_free_scroll);
    assert_eq!(app.sidebar_last_cursor, Some(5));

    app.switch_view(View::PullRequests, &mut Vec::new());

    assert_eq!(app.sidebar_offset, 0);
    assert!(!app.sidebar_free_scroll);
    assert_eq!(app.sidebar_last_cursor, None);
}

#[test]
fn failed_stack_refresh_retains_data_until_a_successful_retry() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_generation = 4;
    app.pull_request = Some(pull_request(42, "Root", "acme/widget"));
    app.pull_request_stack = Some(pull_request_stack(2));

    app.handle_worker_event(
        WorkerEvent::PullRequestStack {
            generation: 4,
            result: Err("stack metadata refresh failed".to_owned()),
        },
        Instant::now(),
    );

    assert!(app.pull_request_stack.is_some());
    assert_eq!(
        app.pull_request_stack_error.as_deref(),
        Some("stack metadata refresh failed")
    );
    assert!(app.pull_request_warnings.iter().any(|warning| {
        warning == "Unable to load pull-request stack: stack metadata refresh failed"
    }));

    app.handle_worker_event(
        WorkerEvent::PullRequestStack {
            generation: 4,
            result: Ok(crate::git::github::PullRequestStackSnapshot {
                stack: Some(pull_request_stack(2)),
                warnings: Vec::new(),
                from_cache: false,
            }),
        },
        Instant::now(),
    );

    assert!(app.pull_request_stack_error.is_none());
    assert!(
        app.pull_request_warnings
            .iter()
            .all(|warning| !warning.starts_with("Unable to load pull-request stack:"))
    );
}

#[test]
fn stale_stack_cache_keeps_the_gate_error_until_network_recovery() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_generation = 4;
    app.pull_request = Some(pull_request(42, "Root", "acme/widget"));

    app.handle_worker_event(
        WorkerEvent::PullRequestStack {
            generation: 4,
            result: Ok(crate::git::github::PullRequestStackSnapshot {
                stack: Some(pull_request_stack(2)),
                warnings: vec![
                    "GitHub is unavailable; showing stale cached stack data for #42".to_owned(),
                ],
                from_cache: true,
            }),
        },
        Instant::now(),
    );

    assert!(
        app.pull_request_stack_error
            .as_deref()
            .is_some_and(|error| error.contains("stale cached stack data"))
    );

    let current = app.pull_request_stack.clone();
    app.handle_worker_event(
        WorkerEvent::PullRequestStack {
            generation: 4,
            result: Ok(crate::git::github::PullRequestStackSnapshot {
                stack: None,
                warnings: vec![
                    "GitHub is unavailable; showing stale cached stack data for #42".to_owned(),
                ],
                from_cache: true,
            }),
        },
        Instant::now(),
    );

    assert_eq!(app.pull_request_stack, current);
}
