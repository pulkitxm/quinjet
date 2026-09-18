use super::*;

#[test]
fn slash_opens_search_with_name_contents_and_both() {
    let mut app = app_with_changes();
    let now = Instant::now();
    drop(app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now));
    assert!(matches!(
        app.modal,
        Some(Modal::Prompt {
            kind: PromptKind::Filter { mode, .. },
            ..
        }) if mode == SearchMode::Name
    ));
    drop(app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), now));
    assert_eq!(app.search_mode, SearchMode::Contents);
    drop(app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), now));
    assert_eq!(app.search_mode, SearchMode::Both);
    drop(app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now));
    assert_eq!(app.search_mode, SearchMode::Name);
}

#[test]
fn typing_filters_the_list_before_enter() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = app_with_changes();
    let now = Instant::now();
    app.open_list_search();
    for character in "README".chars() {
        drop(app.handle_key(
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
            now,
        ));
    }
    assert!(app.modal.is_some());
    assert_eq!(app.filter, "README");
    assert_eq!(app.visible_change_indices(), vec![1]);
    assert_eq!(
        app.search_header_status().as_deref(),
        Some("  Search: README  [Name]  1 result")
    );
    let mut terminal = Terminal::new(TestBackend::new(160, 24)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let sidebar = app.geometry.sidebar;
    let buffer = terminal.backend().buffer();
    let mut rendered = String::new();
    for y in sidebar.y..sidebar.bottom() {
        for x in sidebar.x..sidebar.right() {
            rendered.push_str(buffer[(x, y)].symbol());
        }
    }
    assert!(rendered.contains("README.md"));
    assert!(!rendered.contains("main.rs"));
}

#[test]
fn contents_search_is_queued_after_typing_without_enter() {
    let mut app = app_with_changes();
    let now = Instant::now();
    app.open_list_search();
    drop(app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), now));
    drop(app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), now));
    assert!(app.search_pending);
    assert!(app.search_header_status().unwrap().contains("searching..."));
    let (effects, changed) = app.tick(now + PREVIEW_DEBOUNCE + Duration::from_millis(1));
    assert!(changed);
    assert!(effects.iter().any(|effect| matches!(effect, AppEffect::Git(command) if matches!(**command, WorkerCommand::Search { .. }))));
    app.apply_search_hits(
        app.search_generation,
        Ok(crate::search::SearchHits::empty(SearchMode::Contents, "x")
            .with_paths(vec!["src/main.rs".to_owned()])),
        now + PREVIEW_DEBOUNCE,
    );
    assert!(app.modal.is_some());
    assert_eq!(app.visible_change_indices(), vec![0]);
    assert!(app.search_header_status().unwrap().contains("1 result"));
}

#[test]
fn shift_tab_cycles_search_mode_backward() {
    let mut app = app_with_changes();
    let now = Instant::now();
    app.open_list_search();
    drop(app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT), now));
    assert_eq!(app.search_mode, SearchMode::Both);
    drop(app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), now));
    assert_eq!(app.search_mode, SearchMode::Name);
    drop(app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT), now));
    assert_eq!(app.search_mode, SearchMode::Both);
}

#[test]
fn cancelling_restores_a_contents_query_and_restarts_its_search() {
    let mut app = app_with_changes();
    app.filter = "original".to_owned();
    app.search_mode = SearchMode::Contents;
    let now = Instant::now();
    app.open_list_search();
    app.filter = "other".to_owned();
    app.search_mode = SearchMode::Name;
    drop(app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now));
    assert_eq!(app.filter, "original");
    assert_eq!(app.search_mode, SearchMode::Contents);
    assert!(app.search_pending);
    assert!(app.search_due.is_some());
}

#[test]
fn both_mode_keeps_name_matches_while_contents_are_pending() {
    let mut app = app_with_changes();
    app.filter = "read".to_owned();
    app.search_mode = SearchMode::Both;
    app.search_pending = true;
    assert_eq!(app.visible_change_indices(), vec![1]);
    app.content_hits.extend(["src/main.rs".to_owned()]);
    app.search_pending = false;
    assert_eq!(app.visible_change_indices(), vec![0, 1]);
}

#[test]
fn switching_views_ignores_a_pending_search_reply() {
    let mut app = app_with_changes();
    app.filter = "needle".to_owned();
    app.search_mode = SearchMode::Contents;
    app.schedule_search(Instant::now());
    let old_generation = app.search_generation;
    app.switch_view(View::History, &mut Vec::new());
    app.apply_search_hits(
        old_generation,
        Ok(
            crate::search::SearchHits::empty(SearchMode::Contents, "needle")
                .with_paths(vec!["wrong.txt".to_owned()]),
        ),
        Instant::now(),
    );
    assert!(app.content_hits.is_empty());
    assert_ne!(app.search_generation, old_generation);
}

#[test]
fn pull_request_slash_still_opens_numeric_lookup_outside_files() {
    let mut app = app_with_changes();
    app.view = View::PullRequests;
    let now = Instant::now();
    drop(app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now));
    assert!(app.pull_request_lookup_active);
    assert!(app.modal.is_none());
}
