use super::*;

#[test]
fn a_remote_session_copies_a_clicked_link_instead_of_opening_it() {
    let mut app = app_with_changes();
    app.local_browser = false;
    app.geometry.link_hits = vec![LinkHit {
        area: Rect::new(30, 4, 7, 1),
        target: OpenTarget::Browser("https://github.com/acme/widget/commit/abc".to_owned()),
    }];

    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 32,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Copy(url)] if url == "https://github.com/acme/widget/commit/abc"
    ));
    assert!(app.toast.as_ref().is_some_and(|toast| {
        toast
            .message
            .contains("https://github.com/acme/widget/commit/abc")
            && toast.message.contains("Cmd-click or Ctrl-click")
    }));
}

#[test]
fn a_local_session_opens_a_clicked_link_and_says_so() {
    let mut app = app_with_changes();
    app.geometry.link_hits = vec![LinkHit {
        area: Rect::new(30, 4, 7, 1),
        target: OpenTarget::Browser("https://github.com/acme/widget/commit/abc".to_owned()),
    }];

    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 32,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );

    assert!(app.local_browser);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Open(OpenTarget::Browser(url))]
            if url == "https://github.com/acme/widget/commit/abc"
    ));
    assert!(
        app.toast.as_ref().is_some_and(
            |toast| toast.message == "Opening https://github.com/acme/widget/commit/abc"
        )
    );
}

#[test]
fn the_keyboard_open_action_follows_the_same_local_and_remote_split() {
    let now = Instant::now();
    let mut app = app_with_changes();
    app.status.branch.head = "feature/link".to_owned();
    app.local_github_repository = Some(GitHubRepository {
        name_with_owner: "acme/widget".to_owned(),
        url: "https://github.com/acme/widget".to_owned(),
        remotes: vec!["origin".to_owned()],
    });

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Open(OpenTarget::Browser(url))]
            if url == "https://github.com/acme/widget/tree/feature/link"
    ));
    assert!(
        app.toast
            .as_ref()
            .is_some_and(|toast| toast.message.starts_with("Opening "))
    );

    app.local_browser = false;
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Copy(url)] if url == "https://github.com/acme/widget/tree/feature/link"
    ));
    assert!(
        app.toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("Cmd-click or Ctrl-click"))
    );
}
