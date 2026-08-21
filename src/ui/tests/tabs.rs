use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;
use crate::app::{RepositoryTabDrag, RepositoryTabMenu};
use crate::tabs::{RepositoryTabs, TabId};

fn app_with_tabs(count: usize, active: usize) -> (App, Vec<TabId>) {
    let mut tabs = RepositoryTabs::new("project-0", "/repo-0", ());
    let mut ids = vec![tabs.active_id().expect("initial tab")];
    for index in 1..count {
        ids.push(tabs.append(format!("project-{index}"), format!("/repo-{index}"), ()));
    }
    assert!(tabs.activate(ids[active]));
    let mut app = App::new("/repo-0", "project-0");
    app.set_repository_tabs(tabs.infos());
    (app, ids)
}

#[test]
fn repository_tab_hits_follow_repository_order() {
    let (mut app, ids) = app_with_tabs(3, 1);
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let hits = &app.geometry.repository_tab_hits;
    assert_eq!(hits.iter().map(|hit| hit.id).collect::<Vec<_>>(), ids);
    assert!(
        hits.iter()
            .all(|hit| hit.area.y == 0 && hit.area.height == 1)
    );
    assert!(hits.iter().all(|hit| {
        hit.close.width == 3
            && hit.close.right() == hit.area.right().saturating_sub(1)
            && hit.area.contains((hit.close.x, hit.close.y).into())
    }));
    assert!(hits.windows(2).all(|pair| {
        let [left, right] = pair else {
            return false;
        };
        left.area.right() <= right.area.x
    }));
    assert_eq!(app.geometry.repository_tab_open.right(), 120);
    assert!(
        hits.last()
            .is_some_and(|hit| hit.area.right() <= app.geometry.repository_tab_open.x)
    );
}

#[test]
fn repository_tabs_render_close_icons_and_a_drag_destination() {
    let (mut app, ids) = app_with_tabs(3, 0);
    app.repository_tab_drag = Some(RepositoryTabDrag {
        id: ids[0],
        target: Some(ids[2]),
    });
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let row = terminal.backend().buffer().content()[..120]
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert_eq!(row.matches('×').count(), 3);
    assert!(row.contains('▏'));
}

#[test]
fn repository_tab_strip_has_a_continuous_bottom_separator() {
    let (mut app, _) = app_with_tabs(2, 1);
    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();

    for column in 0..80 {
        let cell = &terminal.backend().buffer()[(column, 1)];
        assert_eq!(cell.symbol(), "─");
        assert_eq!(cell.fg, theme.border);
    }
    assert_eq!(app.geometry.changes_tab.y, 2);
}

#[test]
fn repository_tab_strip_appears_only_for_multiple_tabs() {
    let (mut single, _) = app_with_tabs(1, 0);
    let mut single_terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    single_terminal
        .draw(|frame| draw(frame, &mut single, &Theme::default()))
        .unwrap();

    assert_eq!(single.geometry.repository_tab_hits, Vec::new());
    assert_eq!(single.geometry.repository_tab_open, Rect::default());
    assert_eq!(single.geometry.repository_tab_previous, Rect::default());
    assert_eq!(single.geometry.repository_tab_next, Rect::default());
    assert_eq!(single.geometry.changes_tab.y, 0);

    let (mut multiple, ids) = app_with_tabs(2, 1);
    let mut multiple_terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    multiple_terminal
        .draw(|frame| draw(frame, &mut multiple, &Theme::default()))
        .unwrap();

    assert_eq!(
        multiple
            .geometry
            .repository_tab_hits
            .iter()
            .map(|hit| hit.id)
            .collect::<Vec<_>>(),
        ids
    );
    assert_eq!(
        multiple.geometry.repository_tab_open,
        Rect::new(75, 0, 5, 1)
    );
    assert_eq!(multiple.geometry.repository_tab_previous, Rect::default());
    assert_eq!(multiple.geometry.repository_tab_next, Rect::default());
    assert_eq!(multiple.geometry.changes_tab.y, 2);
}

#[test]
fn overflowing_repository_tabs_keep_the_active_tab_visible() {
    let (mut app, ids) = app_with_tabs(10, 8);
    let mut terminal = Terminal::new(TestBackend::new(72, 18)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let visible = app
        .geometry
        .repository_tab_hits
        .iter()
        .map(|hit| hit.id)
        .collect::<Vec<_>>();
    assert_eq!(visible, ids[4..9]);
    assert!(visible.contains(&ids[8]));
    assert!(!visible.contains(&ids[9]));
    assert_eq!(app.geometry.repository_tab_previous, Rect::new(0, 0, 3, 1));
    assert_eq!(app.geometry.repository_tab_next, Rect::new(64, 0, 3, 1));
}

#[test]
fn repository_tab_menu_registers_four_clamped_actions() {
    let (mut app, ids) = app_with_tabs(2, 0);
    app.repository_tab_menu = Some(RepositoryTabMenu {
        id: ids[0],
        column: 79,
        row: 23,
        selected: 2,
    });
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let hits = &app.geometry.repository_tab_menu_hits;
    assert_eq!(hits.len(), RepositoryTabAction::ALL.len());
    assert_eq!(
        hits.iter().map(|(_, action)| *action).collect::<Vec<_>>(),
        RepositoryTabAction::ALL.to_vec()
    );
    assert_eq!(
        hits.first().map(|(area, _)| *area),
        Some(Rect::new(59, 19, 20, 1))
    );
    assert_eq!(
        hits.last().map(|(area, _)| *area),
        Some(Rect::new(59, 22, 20, 1))
    );
    assert!(
        hits.iter()
            .all(|(area, _)| area.right() <= 80 && area.bottom() <= 24)
    );
}

#[test]
fn new_tab_picker_names_its_destination() {
    let mut app = App::new("/repo", "project");
    drop(app.handle_key(
        KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT),
        Instant::now(),
    ));
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(rendered.contains("Open in new tab"));
}
