use super::*;
use crate::convert::cells;
use crate::tabs::{RepositoryTabs, TabId};

fn repository_tabs() -> (RepositoryTabs<()>, TabId, TabId, TabId) {
    let mut tabs = RepositoryTabs::new("one", "/one", ());
    let first = tabs.active_id().expect("initial tab is active");
    let second = tabs.append("two", "/two", ());
    let third = tabs.append("three", "/three", ());
    assert!(tabs.activate(first));
    (tabs, first, second, third)
}

fn app_with_repository_tabs(tabs: &RepositoryTabs<()>) -> App {
    let mut app = App::new("/one", "one");
    app.set_repository_tabs(tabs.infos());
    app.geometry.repository_tab_hits = app
        .repository_tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| RepositoryTabHit {
            area: Rect::new(cells(index.saturating_mul(12)), 0, 12, 1),
            close: Rect::new(cells(index.saturating_mul(12).saturating_add(8)), 0, 3, 1),
            id: tab.id,
        })
        .collect();
    app.geometry.repository_tab_open = Rect::new(36, 0, 5, 1);
    app
}

const fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn project_group() -> ProjectGroup {
    ProjectGroup {
        name: "target".to_owned(),
        common_dir: PathBuf::from("/target/.git"),
        worktrees: vec![Worktree {
            path: PathBuf::from("/target"),
            head: "0123456789abcdef".to_owned(),
            branch: Some("main".to_owned()),
            current: false,
            bare: false,
            detached: false,
            locked: None,
            prunable: None,
        }],
    }
}

#[test]
fn project_shortcuts_choose_current_or_new_tab_mode() {
    let now = Instant::now();
    let mut app = App::new("/one", "one");
    app.project_groups = vec![project_group()];

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE), now);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadRecentProjects { .. })
    )));
    assert!(matches!(
        app.modal,
        Some(Modal::Projects {
            mode: ProjectOpenMode::CurrentTab,
            ..
        })
    ));
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::SwitchRepository(path)] if path == Path::new("/target")
    ));

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT), now);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadRecentProjects { .. })
    )));
    assert!(matches!(
        app.modal,
        Some(Modal::Projects {
            mode: ProjectOpenMode::NewTab,
            ..
        })
    ));
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::OpenRepositoryTab(path)] if path == Path::new("/target")
    ));
}

#[test]
fn control_tab_cycles_and_control_w_closes_the_active_repository_tab() {
    let (mut tabs, first, second, third) = repository_tabs();
    let mut app = app_with_repository_tabs(&tabs);
    let now = Instant::now();

    let effects = app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::ActivateRepositoryTab(id)] if *id == second
    ));

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL | KeyModifiers::SHIFT),
        now,
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::ActivateRepositoryTab(id)] if *id == third
    ));

    let effects = app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::CONTROL), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::ActivateRepositoryTab(id)] if *id == third
    ));

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        now,
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::CloseRepositoryTab(id)] if *id == first
    ));

    assert!(tabs.activate(second));
    app.set_repository_tabs(tabs.infos());
    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        now,
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::CloseRepositoryTab(id)] if *id == second
    ));
}

#[test]
fn control_w_edits_an_open_text_modal_instead_of_closing_its_tab() {
    let (tabs, ..) = repository_tabs();
    let mut app = app_with_repository_tabs(&tabs);
    app.modal = Some(Modal::Commit {
        input: TextBuffer::new("keep this draft"),
        amend: false,
    });

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert!(matches!(
        app.modal,
        Some(Modal::Commit { input, .. }) if input.value == "keep this "
    ));
}

#[test]
fn clicking_a_repository_tab_activates_it_on_release() {
    let (tabs, _, second, _) = repository_tabs();
    let mut app = app_with_repository_tabs(&tabs);
    let now = Instant::now();

    let effects = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 14, 0), now);
    assert!(effects.is_empty());
    assert_eq!(
        app.repository_tab_drag,
        Some(RepositoryTabDrag {
            id: second,
            target: None,
        })
    );

    let effects = app.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 14, 0), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::ActivateRepositoryTab(id)] if *id == second
    ));
    assert_eq!(app.repository_tab_drag, None);
}

#[test]
fn overflow_controls_cycle_to_hidden_repository_tabs() {
    let (tabs, _, second, third) = repository_tabs();
    let mut app = app_with_repository_tabs(&tabs);
    app.geometry.repository_tab_previous = Rect::new(50, 0, 3, 1);
    app.geometry.repository_tab_next = Rect::new(53, 0, 3, 1);
    let now = Instant::now();

    let effects = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 51, 0), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::ActivateRepositoryTab(id)] if *id == third
    ));

    let effects = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 54, 0), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::ActivateRepositoryTab(id)] if *id == second
    ));
}

#[test]
fn dragging_a_repository_tab_reorders_without_activating_on_release() {
    let (tabs, first, _, third) = repository_tabs();
    let mut app = app_with_repository_tabs(&tabs);
    let now = Instant::now();

    assert!(
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 2, 0), now)
            .is_empty()
    );
    let effects = app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 26, 0), now);
    assert!(effects.is_empty());
    assert_eq!(
        app.repository_tab_drag,
        Some(RepositoryTabDrag {
            id: first,
            target: Some(third),
        })
    );

    let effects = app.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 26, 0), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::ReorderRepositoryTab { source, target }]
            if *source == first && *target == third
    ));
    assert_eq!(app.repository_tab_drag, None);
}

#[test]
fn clicking_a_repository_tab_close_icon_closes_that_tab() {
    let (tabs, _, second, _) = repository_tabs();
    let mut app = app_with_repository_tabs(&tabs);

    let effects = app.handle_mouse(
        mouse(MouseEventKind::Down(MouseButton::Left), 21, 0),
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::CloseRepositoryTab(id)] if *id == second
    ));
    assert_eq!(app.repository_tab_drag, None);
}

#[test]
fn right_click_menu_routes_close_others_and_dismisses_outside() {
    let (tabs, _, second, _) = repository_tabs();
    let mut app = app_with_repository_tabs(&tabs);
    let now = Instant::now();

    let effects = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), 14, 0), now);
    assert!(effects.is_empty());
    assert_eq!(
        app.repository_tab_menu,
        Some(RepositoryTabMenu {
            id: second,
            column: 14,
            row: 0,
            selected: 0,
        })
    );

    app.geometry.repository_tab_menu_hits =
        vec![(Rect::new(14, 3, 20, 1), RepositoryTabAction::CloseOthers)];
    let effects = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 18, 3), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::CloseOtherRepositoryTabs(id)] if *id == second
    ));
    assert_eq!(app.repository_tab_menu, None);

    drop(app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), 14, 0), now));
    assert!(app.repository_tab_menu.is_some());
    let effects = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), 70, 8), now);
    assert!(effects.is_empty());
    assert_eq!(app.repository_tab_menu, None);
}

#[test]
fn right_click_menu_opens_projects_and_routes_close_all() {
    let (tabs, _, second, _) = repository_tabs();
    let mut app = app_with_repository_tabs(&tabs);
    let now = Instant::now();

    drop(app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), 14, 0), now));
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        app.modal,
        Some(Modal::Projects {
            mode: ProjectOpenMode::NewTab,
            ..
        })
    ));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadRecentProjects { .. })
    )));
    assert_eq!(app.repository_tab_menu, None);

    app.modal = None;
    drop(app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), 14, 0), now));
    let effects = app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now);
    assert!(effects.is_empty());
    assert_eq!(
        app.repository_tab_menu,
        Some(RepositoryTabMenu {
            id: second,
            column: 14,
            row: 0,
            selected: 3,
        })
    );
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::CloseAllRepositoryTabs]
    ));
    assert_eq!(app.repository_tab_menu, None);
}

#[test]
fn repository_tab_commands_leave_each_app_view_state_independent() {
    let (mut tabs, first, second, _) = repository_tabs();
    let now = Instant::now();
    let mut first_app = app_with_repository_tabs(&tabs);
    first_app.view = View::PullRequests;
    first_app.content_scroll = 47;
    first_app.horizontal_scroll = 9;

    assert!(tabs.activate(second));
    let mut second_app = app_with_repository_tabs(&tabs);
    second_app.view = View::History;
    second_app.history_cursor = 6;
    second_app.content_scroll = 18;
    second_app.set_tab_active(false, now);

    let effects = first_app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::ActivateRepositoryTab(id)] if *id == second
    ));
    assert_eq!(first_app.view, View::PullRequests);
    assert_eq!(first_app.content_scroll, 47);
    assert_eq!(first_app.horizontal_scroll, 9);
    assert_eq!(second_app.view, View::History);
    assert_eq!(second_app.history_cursor, 6);
    assert_eq!(second_app.content_scroll, 18);

    assert!(tabs.activate(first));
    first_app.set_repository_tabs(tabs.infos());
    assert_eq!(first_app.view, View::PullRequests);
    assert_eq!(first_app.content_scroll, 47);
}
