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
