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
fn start_truncation_keeps_the_nearest_directories() {
    assert_eq!(truncate_start("apps/server/src/realtime", 10), "…/realtime");
    assert_eq!(truncate_start("src", 10), "src");
    assert_eq!(truncate_start("src/api", 0), "");
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
        ("presence.ts".to_owned(), "…c/realtime".to_owned())
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
        fit_file_label("conversation-presence.ts", "apps/server", 12),
        ("convers…e.ts".to_owned(), String::new())
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

    assert!(rendered.contains("sessions.ts  …"), "{rendered}");
    assert!(rendered.contains("messages.ts  …"), "{rendered}");
    assert!(rendered.contains("presence.test.ts  …"), "{rendered}");
    assert!(rendered.contains("/routes"), "{rendered}");
    assert!(rendered.contains("/chat"), "{rendered}");
    assert!(!rendered.contains("apps/server"), "{rendered}");
}

#[test]
fn wide_changes_sidebar_shows_the_whole_directory() {
    let rendered = sidebar_rows(vec![unstaged("apps/server/src/chat/messages.ts")], 60).join("\n");

    assert!(
        rendered.contains("messages.ts  apps/server/src/chat"),
        "{rendered}"
    );
}
