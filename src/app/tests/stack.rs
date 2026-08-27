use super::*;

#[test]
fn stack_snapshot_defaults_to_the_selected_member_and_preserves_an_extended_range() {
    let now = Instant::now();
    let mut app = App::new("/tmp/repo", "repo");
    let stack = pull_request_stack(3);

    app.apply_pull_request_stack_snapshot(Some(stack.clone()), &mut Vec::new());
    assert_eq!(app.pull_request_stack_range(), Some((3, 3)));
    assert!(app.select_pull_request_stack_member(2, true, now));
    assert_eq!(app.pull_request_stack_range(), Some((2, 3)));

    app.apply_pull_request_stack_snapshot(Some(stack), &mut Vec::new());
    assert_eq!(app.pull_request_stack_range(), Some((2, 3)));
    assert_eq!(app.pull_request_stack_anchor, Some(3));
    assert_eq!(app.pull_request_stack_cursor, Some(2));
}

#[test]
fn stack_keyboard_and_mouse_selection_control_the_composed_range() {
    let now = Instant::now();
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(43, "Layer 3", "acme/widget"));
    app.pull_request_stack = Some(pull_request_stack(3));
    app.pull_request_stack_anchor = Some(3);
    app.pull_request_stack_cursor = Some(3);
    app.pull_request_section = PullRequestSection::Stack;

    drop(app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT), now));
    assert_eq!(app.pull_request_stack_range(), Some((2, 3)));

    app.geometry.sidebar = Rect::new(0, 0, 30, 10);
    app.geometry.sidebar_hits = vec![SidebarHitArea {
        area: Rect::new(1, 2, 20, 1),
        target: SidebarHit::PullRequestStackMember(1),
    }];
    drop(app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        now,
    ));
    assert_eq!(app.pull_request_stack_range(), Some((1, 3)));

    drop(app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now));
    assert_eq!(app.pull_request_stack_range(), Some((2, 2)));
}

#[test]
fn stack_range_prepares_its_own_diff_and_member_url() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(43, "Layer 3", "acme/widget"));
    app.pull_request_stack = Some(pull_request_stack(3));
    app.pull_request_stack_anchor = Some(1);
    app.pull_request_stack_cursor = Some(3);
    app.pull_request_section = PullRequestSection::Stack;
    let mut effects = Vec::new();

    app.request_preview(&mut effects);

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::PreparePullRequestStack { from: 1, to: 3, .. }
        )
    ));
    assert_eq!(
        app.github_url_for_selection(),
        Some("https://github.com/acme/widget/pull/43")
    );
}

#[test]
fn stale_stack_metadata_does_not_replace_the_active_lookup() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_generation = 2;
    app.pull_request_stack_loading = true;

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestStack {
            generation: 1,
            result: Ok(crate::git::github::PullRequestStackSnapshot {
                stack: Some(pull_request_stack(3)),
                warnings: Vec::new(),
                from_cache: false,
            }),
        },
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert!(app.pull_request_stack_loading);
    assert!(app.pull_request_stack.is_none());
}

#[test]
fn stack_member_identity_survives_reordering_and_inspector_reset_is_isolated() {
    let mut app = App::new("/tmp/repo", "repo");
    let mut stack = pull_request_stack(2);
    let identity = stack.member_identity(2).expect("member identity");
    let locator = stack.member_pull_request(2).expect("member locator");
    assert!(app.stack_inspector.select(identity.clone(), locator));
    app.stack_inspector.selected_loading = true;
    let root_generation = app.pull_request_generation;
    let selected_generation = app.stack_inspector.selected_generation;

    if let Some(member) = stack.members.iter_mut().find(|member| member.position == 2) {
        member.position = 7;
    }
    assert_eq!(stack.member_identity(7), Some(identity));

    app.reset_pull_request_runtime();

    assert!(app.stack_inspector.selected_identity.is_none());
    assert!(!app.stack_inspector.selected_loading);
    assert_ne!(app.stack_inspector.selected_generation, selected_generation);
    assert_eq!(app.pull_request_generation, root_generation);
}

#[test]
fn stack_snapshot_requests_only_the_selected_summary_and_tip_checks() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request = Some(pull_request(42, "Root", "acme/widget"));
    app.pull_request_checks = vec![check("root", PullRequestCheckStatus::Passed)];
    let root_checks = app.pull_request_checks.clone();
    let mut effects = Vec::new();

    app.apply_pull_request_stack_snapshot(Some(pull_request_stack(2)), &mut effects);

    assert_eq!(effects.len(), 2);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestStackMember {
                identity,
                ..
            } if identity.number == 42)
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestStackTipChecks {
                identity,
                ..
            } if identity.number == 43)
    )));
    assert_eq!(app.pull_request_checks, root_checks);
}

#[test]
fn stack_section_switch_fetches_only_its_member_stream() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_stack = Some(pull_request_stack(2));
    app.pull_request_stack_cursor = Some(2);
    app.reconcile_stack_inspector();
    app.stack_inspector.tip_checks_loading = true;
    let mut effects = Vec::new();

    app.select_stack_member_section(StackMemberSection::Conversation, &mut effects);

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestStackMemberConversation { identity, .. }
                if identity.number == 42
        )
    ));
}

#[test]
fn stack_member_events_require_identity_and_generation_without_touching_root_state() {
    let mut app = App::new("/tmp/repo", "repo");
    let stack = pull_request_stack(2);
    let identity = stack.member_identity(2).expect("identity");
    let stale_identity = stack.member_identity(1).expect("stale identity");
    let locator = stack.member_pull_request(2).expect("locator");
    let _ = app.stack_inspector.select(identity.clone(), locator);
    app.stack_inspector.checks_generation = 7;
    app.stack_inspector.checks_loading = true;
    app.pull_request_checks = vec![check("root", PullRequestCheckStatus::Passed)];
    let root_checks = app.pull_request_checks.clone();

    drop(app.handle_worker_event(
        WorkerEvent::PullRequestStackMemberChecks {
            identity: stale_identity,
            generation: 7,
            result: Ok(crate::git::github::PullRequestChecks::default()),
        },
        Instant::now(),
    ));
    assert!(app.stack_inspector.checks_loading);

    drop(app.handle_worker_event(
        WorkerEvent::PullRequestStackMemberChecks {
            identity: identity.clone(),
            generation: 6,
            result: Ok(crate::git::github::PullRequestChecks::default()),
        },
        Instant::now(),
    ));
    assert!(app.stack_inspector.checks_loading);

    drop(app.handle_worker_event(
        WorkerEvent::PullRequestStackMemberChecks {
            identity,
            generation: 7,
            result: Ok(crate::git::github::PullRequestChecks {
                checks: vec![check("member", PullRequestCheckStatus::Failed)],
                from_cache: false,
            }),
        },
        Instant::now(),
    ));
    assert!(!app.stack_inspector.checks_loading);
    assert!(app.stack_inspector.checks_loaded);
    assert_eq!(app.stack_inspector.checks.checks[0].name, "member");
    assert_eq!(app.pull_request_checks, root_checks);
}
