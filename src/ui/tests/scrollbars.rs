use std::time::Instant;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;

fn render(app: &mut App, terminal: &mut Terminal<TestBackend>) {
    terminal
        .draw(|frame| draw(frame, app, &Theme::default()))
        .unwrap();
}

fn mouse(app: &mut App, kind: MouseEventKind, column: u16, row: u16) -> Vec<crate::app::AppEffect> {
    app.handle_mouse(
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    )
}

fn document_app(length: usize) -> App {
    let mut app = App::new("/tmp/repo", "repo");
    app.set_document(DiffDocument {
        lines: (0..length)
            .map(|index| test_line(DiffLineKind::Context, &format!("line {index}")))
            .collect(),
        ..DiffDocument::default()
    });
    app
}

#[test]
fn scrollbar_drag_scrolls_both_diff_layouts_across_redraws_and_releases_without_copying() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = document_app(400);
        app.diff_layout = layout;
        let mut terminal = Terminal::new(TestBackend::new(160, 32)).unwrap();
        render(&mut app, &mut terminal);
        let scrollbar = app.geometry.content_scrollbar.unwrap();
        assert_eq!(
            terminal.backend().buffer()[(scrollbar.thumb.x, scrollbar.thumb.y)].symbol(),
            "▐"
        );
        assert!(
            mouse(
                &mut app,
                MouseEventKind::Down(MouseButton::Left),
                scrollbar.thumb.x,
                scrollbar.thumb.y,
            )
            .is_empty()
        );
        assert_eq!(app.content_scroll, 0);
        assert_eq!(app.focus, Focus::Content);
        assert!(app.text_selection.is_none());
        drop(mouse(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            2,
            scrollbar.track.y + 10,
        ));
        render(&mut app, &mut terminal);
        assert!(app.content_scroll > 0);
        assert!(app.content_scroll < scrollbar.max_offset);
        assert!(
            rendered_text(terminal.backend().buffer())
                .contains(&format!("line {}", app.content_scroll))
        );
        drop(mouse(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            2,
            u16::MAX,
        ));
        render(&mut app, &mut terminal);
        assert_eq!(app.content_scroll, scrollbar.max_offset);
        assert!(app.content_at_bottom);
        assert!(mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 2, 0,).is_empty());
        render(&mut app, &mut terminal);
        assert_eq!(app.content_scroll, 0);
        assert!(app.scrollbar_drag.is_none());
        assert!(app.text_selection.is_none());
        drop(mouse(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            scrollbar.track.x,
            scrollbar.track.bottom(),
        ));
        assert_eq!(app.content_scroll, 0);
    }
}

#[test]
fn grabbing_inside_a_large_thumb_preserves_offset_and_grab_position() {
    let mut app = document_app(40);
    app.content_scroll = 5;
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    render(&mut app, &mut terminal);
    let scrollbar = app.geometry.content_scrollbar.unwrap();
    let row = scrollbar.thumb.bottom() - 1;
    drop(mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        scrollbar.thumb.x,
        row,
    ));
    render(&mut app, &mut terminal);
    drop(mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        scrollbar.thumb.x,
        row,
    ));
    assert_eq!(app.content_scroll, 5);
    drop(mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        scrollbar.thumb.x,
        row,
    ));
    drop(mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        scrollbar.thumb.x,
        row + 2,
    ));
    render(&mut app, &mut terminal);
    let moved = app.geometry.content_scrollbar.unwrap();
    assert_eq!(moved.thumb.y, scrollbar.thumb.y + 2);
}

#[test]
fn clicking_the_track_scrolls_and_can_continue_as_a_drag() {
    let mut app = document_app(400);
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    render(&mut app, &mut terminal);
    let scrollbar = app.geometry.content_scrollbar.unwrap();
    drop(mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        scrollbar.track.x,
        scrollbar.track.bottom() - 1,
    ));
    render(&mut app, &mut terminal);
    assert_eq!(app.content_scroll, scrollbar.max_offset);
    drop(mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        scrollbar.track.x,
        scrollbar.track.y,
    ));
    render(&mut app, &mut terminal);
    assert_eq!(app.content_scroll, 0);
    assert!(app.text_selection.is_none());
}

#[test]
fn change_list_scrollbar_does_not_select_or_stage_files() {
    let mut app = App::new("/tmp/repo", "repo");
    app.status.changes = (0..80)
        .map(|index| Change {
            path: format!("file-{index:02}.rs").into(),
            original_path: None,
            area: ChangeArea::Unstaged,
            status: ChangeStatus::Modified,
        })
        .collect();
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    render(&mut app, &mut terminal);
    let scrollbar = app.geometry.sidebar_scrollbar.unwrap();
    let cursor = app.change_cursor;
    assert!(
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            scrollbar.track.x,
            scrollbar.track.bottom() - 1,
        )
        .is_empty()
    );
    render(&mut app, &mut terminal);
    assert_eq!(app.sidebar_offset, scrollbar.max_offset);
    assert_eq!(app.change_cursor, cursor);
    assert_eq!(app.focus, Focus::Sidebar);
    assert!(app.sidebar_free_scroll);
    assert!(rendered_text(terminal.backend().buffer()).contains("file-79.rs"));
    assert!(app.text_selection.is_none());
}

#[test]
fn scrollbars_disappear_when_content_fits_and_cancel_an_active_drag() {
    let mut app = document_app(400);
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    render(&mut app, &mut terminal);
    let scrollbar = app.geometry.content_scrollbar.unwrap();
    drop(mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        scrollbar.thumb.x,
        scrollbar.thumb.y,
    ));
    app.set_document(DiffDocument::empty("short", "fits"));
    render(&mut app, &mut terminal);
    assert!(app.geometry.content_scrollbar.is_none());
    drop(mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        scrollbar.track.x,
        scrollbar.track.bottom(),
    ));
    assert_eq!(app.content_scroll, 0);
    assert!(app.scrollbar_drag.is_none());
}
