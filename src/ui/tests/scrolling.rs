use super::*;

#[test]
fn a_scrolled_content_pane_offers_a_jump_to_top_control() {
    use std::time::Instant;

    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.set_document(DiffDocument {
        title: "Changes".to_owned(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: (0..200)
            .map(|index| test_line(DiffLineKind::Context, &format!("line {index}")))
            .collect(),
    });
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(
        !app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| hit.action == ScmAction::JumpToTop),
        "the control stays away while the reader is at the top"
    );

    app.content_scroll = 40;
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let hit = app
        .geometry
        .scm_action_hits
        .iter()
        .find(|hit| hit.action == ScmAction::JumpToTop)
        .expect("a scrolled document offers the control")
        .clone();
    assert_eq!(hit.area.y, app.geometry.content.y);
    let buffer = terminal.backend().buffer();
    let mut label = String::new();
    for x in hit.area.x..hit.area.right() {
        label.push_str(buffer[(x, hit.area.y)].symbol());
    }
    assert_eq!(label, " ↑ Top ");

    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.area.x,
            row: hit.area.y,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );
    assert_eq!(app.content_scroll, 0);
}

#[test]
fn a_scrollable_content_pane_offers_a_jump_to_bottom_control() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.set_document(DiffDocument {
        title: "Changes".to_owned(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: (0..200)
            .map(|index| test_line(DiffLineKind::Context, &format!("line {index}")))
            .collect(),
    });
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(
        app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| hit.action == ScmAction::JumpToBottom),
        "a long document offers the control"
    );

    app.content_scroll = usize::MAX;
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(
        !app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| hit.action == ScmAction::JumpToBottom),
        "the control disappears once the reader is at the bottom"
    );
}
