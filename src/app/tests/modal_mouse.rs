use super::*;

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn modal_rows_follow_hover_and_activate_on_click() {
    let mut app = App::new("/tmp/repo", "repo");
    let original = app.theme_name;
    app.modal = Some(Modal::Themes {
        selected: 0,
        original,
    });
    app.geometry.modal_list_hits = vec![(Rect::new(10, 5, 20, 1), 1)];
    app.geometry.modal_list_len = ThemeName::ALL.len();

    drop(app.handle_mouse(mouse(MouseEventKind::Moved, 12, 5), Instant::now()));

    assert!(matches!(
        app.modal.as_ref(),
        Some(Modal::Themes { selected: 1, .. })
    ));
    assert_eq!(app.theme_name, ThemeName::ALL[1]);

    drop(app.handle_mouse(
        mouse(MouseEventKind::Down(MouseButton::Left), 12, 5),
        Instant::now(),
    ));

    assert!(app.modal.is_none());
    assert_eq!(app.theme_name, ThemeName::ALL[1]);
}

#[test]
fn modal_wheel_scrolling_does_not_move_selection() {
    let mut app = App::new("/tmp/repo", "repo");
    app.modal = Some(Modal::CommandPalette {
        query: TextBuffer::default(),
        selected: 0,
    });
    app.geometry.modal_list_len = 20;
    app.geometry.modal_list_max_scroll = 12;

    drop(app.handle_mouse(mouse(MouseEventKind::ScrollDown, 20, 10), Instant::now()));

    assert_eq!(app.modal_scroll, 2);
    assert!(app.modal_free_scroll);
    assert!(matches!(
        app.modal.as_ref(),
        Some(Modal::CommandPalette { selected: 0, .. })
    ));

    drop(app.handle_mouse(mouse(MouseEventKind::ScrollUp, 20, 10), Instant::now()));

    assert_eq!(app.modal_scroll, 0);
}

#[test]
fn commit_mode_has_a_keyboard_toggle() {
    let mut app = App::new("/tmp/repo", "repo");
    app.modal = Some(Modal::Commit {
        input: TextBuffer::new("message"),
        amend: false,
    });

    drop(app.handle_key(
        KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        Instant::now(),
    ));

    assert!(matches!(
        app.modal.as_ref(),
        Some(Modal::Commit { amend: true, .. })
    ));
}

#[test]
fn conflict_choices_are_clickable() {
    let mut app = App::new("/tmp/repo", "repo");
    app.modal = Some(Modal::Conflict {
        change: Change {
            path: PathBuf::from("src/main.rs"),
            original_path: None,
            area: ChangeArea::Conflict,
            status: ChangeStatus::Conflicted,
        },
    });
    app.geometry.modal_action_hits = vec![(Rect::new(10, 5, 12, 1), ModalAction::ConflictOurs)];

    let effects = app.handle_mouse(
        mouse(MouseEventKind::Down(MouseButton::Left), 12, 5),
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::Operate {
                operation: GitOperation::ResolveConflict {
                    path,
                    choice: ConflictChoice::Ours,
                },
                ..
            } if path == &PathBuf::from("src/main.rs")
        )
    ));
}
