use super::*;

#[test]
fn stack_review_and_next_buttons_drive_the_selected_member() {
    let now = Instant::now();
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(43, "Layer 3", "acme/widget"));
    app.pull_request_stack = Some(pull_request_stack(1));
    app.pull_request_stack_anchor = Some(1);
    app.pull_request_stack_cursor = Some(1);
    app.pull_request_section = PullRequestSection::Stack;
    app.reconcile_stack_inspector();
    app.geometry.stack_inspector_hits = vec![StackInspectorHitArea {
        area: Rect::new(10, 4, 10, 1),
        target: StackInspectorHit::Review,
    }];

    drop(app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        now,
    ));
    assert!(matches!(
        &app.modal,
        Some(Modal::PullRequestActions { title, .. }) if title == "Submit Review"
    ));

    app.modal = None;
    app.geometry.stack_inspector_hits = vec![StackInspectorHitArea {
        area: Rect::new(22, 4, 10, 1),
        target: StackInspectorHit::Next,
    }];
    drop(app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 24,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        now,
    ));
    assert_eq!(app.pull_request_stack_cursor, Some(2));
}
