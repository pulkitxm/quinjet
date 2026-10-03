use super::*;

fn unstaged(path: &str) -> Change {
    Change {
        path: std::path::PathBuf::from(path),
        original_path: None,
        area: ChangeArea::Unstaged,
        status: ChangeStatus::Modified,
    }
}

fn sidebar_rows(changes: Vec<Change>, width: u16) -> Vec<String> {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.status.changes = changes;
    let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
    terminal
        .draw(|frame| {
            draw_changes_sidebar(frame, frame.area(), &mut app, &Theme::default());
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        })
        .collect()
}

#[test]
fn directories_shorten_by_whole_segments_from_the_start() {
    assert_eq!(
        truncate_directory("apps/server/src/realtime", 14),
        Some("…/src/realtime".to_owned())
    );
    assert_eq!(
        truncate_directory("apps/server/src/realtime", 13),
        Some("…/realtime".to_owned())
    );
    assert_eq!(truncate_directory("src", 3), Some("src".to_owned()));
    assert_eq!(truncate_directory("apps/server/integration", 12), None);
    assert_eq!(truncate_directory("integration", 8), None);
}

#[test]
fn file_labels_shorten_the_directory_before_the_name() {
    assert_eq!(
        fit_file_label("presence.ts", "apps/server/src/realtime", 40),
        (
            "presence.ts".to_owned(),
            "apps/server/src/realtime".to_owned()
        )
    );
    assert_eq!(
        fit_file_label("presence.ts", "apps/server/src/realtime", 24),
        ("presence.ts".to_owned(), "…/realtime".to_owned())
    );
    assert_eq!(
        fit_file_label("presence.ts", "apps/server/src/realtime", 15),
        ("presence.ts".to_owned(), String::new())
    );
    assert_eq!(
        fit_file_label("presence.ts", "src", 16),
        ("presence.ts".to_owned(), "src".to_owned())
    );
    assert_eq!(
        fit_file_label("very-long-component-name.ts", "apps/server", 12),
        ("very-lo…e.ts".to_owned(), String::new())
    );
}

#[test]
fn path_truncation_keeps_the_file_name_whole() {
    assert_eq!(
        truncate_path("apps/server/src/realtime/presence.ts", 36),
        "apps/server/src/realtime/presence.ts"
    );
    assert_eq!(
        truncate_path("apps/server/src/realtime/presence.ts", 22),
        "…/realtime/presence.ts"
    );
    assert_eq!(
        truncate_path("apps/server/src/realtime/presence.ts", 13),
        "…/presence.ts"
    );
    assert_eq!(
        truncate_path("apps/server/src/realtime/presence.ts", 11),
        "presence.ts"
    );
    assert_eq!(
        truncate_path("very-long-component-name.ts", 12),
        "very-lo…e.ts"
    );
}

#[test]
fn narrow_changes_sidebar_keeps_full_file_names() {
    let rows = sidebar_rows(
        vec![
            unstaged("apps/server/src/api/routes/sessions.ts"),
            unstaged("apps/server/src/chat/messages.ts"),
            unstaged("apps/server/integration/presence.test.ts"),
        ],
        40,
    );
    let rendered = rows.join("\n");

    assert!(
        rendered.contains(" sessions.ts  …/api/routes   M [+]"),
        "{rendered}"
    );
    assert!(
        rendered.contains(" messages.ts  …/src/chat     M [+]"),
        "{rendered}"
    );
    assert!(
        rendered.contains(" presence.test.ts            M [+]"),
        "{rendered}"
    );
}

#[test]
fn wide_changes_sidebar_shows_the_whole_directory() {
    let rendered = sidebar_rows(vec![unstaged("apps/server/src/chat/messages.ts")], 60).join("\n");

    assert!(
        rendered.contains("messages.ts  apps/server/src/chat"),
        "{rendered}"
    );
}

#[test]
fn file_header_shortens_the_directory_before_the_name() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let header = test_file_header("apps/server/src/realtime/presence.ts", 12, 3);
    let mut app = App::new("/tmp/repo", "repo");
    app.document.lines = vec![header.clone()];
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    terminal
        .draw(|frame| draw_file_header(frame, frame.area(), &header, &app, &Theme::default()))
        .unwrap();
    let rendered = rendered_text(terminal.backend().buffer());

    assert!(rendered.contains("…/realtime/presence.ts"), "{rendered:?}");
    assert!(rendered.ends_with("+12 -3 ─"), "{rendered:?}");
}
