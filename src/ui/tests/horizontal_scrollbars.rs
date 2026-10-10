use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::scrollbars::{mouse, render};
use super::*;

fn wide_app(layout: DiffLayout) -> App {
    let mut app = App::new("/example/project", "example");
    app.diff_layout = layout;
    app.set_document(DiffDocument {
        lines: std::iter::once(test_file_header("example.rs", 200, 0))
            .chain(std::iter::repeat_n(
                test_line(
                    DiffLineKind::Added,
                    &format!("{}end marker", "界0123456789".repeat(30)),
                ),
                200,
            ))
            .collect(),
        ..DiffDocument::default()
    });
    app
}

#[test]
fn horizontal_grab_keeps_its_position_across_redraws_and_release_outside_the_track() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = wide_app(layout);
        let mut terminal = Terminal::new(TestBackend::new(160, 32)).unwrap();
        app.horizontal_scroll = 57;
        render(&mut app, &mut terminal);
        let bar = app.geometry.horizontal_scrollbar.unwrap();
        let column = bar.thumb.right() - 1;
        assert!(
            mouse(
                &mut app,
                MouseEventKind::Down(MouseButton::Left),
                column,
                bar.thumb.y
            )
            .is_empty()
        );
        render(&mut app, &mut terminal);
        assert!(mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), column, 0).is_empty());
        assert_eq!(app.horizontal_scroll, 57);
        assert!(mouse(&mut app, MouseEventKind::Up(MouseButton::Left), column, 0).is_empty());
        assert_eq!(app.horizontal_scroll, 57);
        assert!(app.scrollbar_drag.is_none());
        drop(mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            column,
            bar.thumb.y,
        ));
        drop(mouse(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            column + 2,
            0,
        ));
        render(&mut app, &mut terminal);
        let moved = app.geometry.horizontal_scrollbar.unwrap();
        assert_eq!(moved.thumb.x, bar.thumb.x + 2);
        assert_eq!(app.content_scroll, 0);
        assert!(app.text_selection.is_none());
    }
}

#[test]
fn horizontal_drag_reaches_both_ends_after_resize_and_mouse_up_applies_the_final_position() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = wide_app(layout);
        let mut terminal = Terminal::new(TestBackend::new(160, 32)).unwrap();
        render(&mut app, &mut terminal);
        let bar = app.geometry.horizontal_scrollbar.unwrap();
        drop(mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            bar.thumb.x,
            bar.thumb.y,
        ));
        terminal.backend_mut().resize(100, 24);
        render(&mut app, &mut terminal);
        let resized = app.geometry.horizontal_scrollbar.unwrap();
        assert!(resized.max_offset > bar.max_offset);
        drop(mouse(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            u16::MAX,
            0,
        ));
        render(&mut app, &mut terminal);
        assert_eq!(app.horizontal_scroll, resized.max_offset);
        assert!(rendered_text(terminal.backend().buffer()).contains("end marker"));
        assert!(mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 0, 0).is_empty());
        render(&mut app, &mut terminal);
        assert_eq!(app.horizontal_scroll, 0);
        assert!(app.scrollbar_drag.is_none());
        assert!(app.text_selection.is_none());
    }
}

#[test]
fn horizontal_drags_cancel_when_keyboard_overlay_mouse_capture_or_content_changes() {
    for transition in 0..7 {
        let mut app = wide_app(DiffLayout::Unified);
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        render(&mut app, &mut terminal);
        let bar = app.geometry.horizontal_scrollbar.unwrap();
        drop(mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            bar.thumb.x,
            bar.thumb.y,
        ));
        assert!(app.scrollbar_drag.is_some());
        match transition {
            0 => {
                drop(app.handle_key(
                    KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
                    Instant::now(),
                ));
            }
            1 => {
                app.modal = Some(Modal::Help {
                    selected: 0,
                    scroll: 0,
                    hover: None,
                });
            }
            2 => app.scm_menu_open = true,
            3 => app.pr_menu_open = true,
            4 => app.configure_mouse_capture(false),
            5 => terminal.backend_mut().resize(60, 12),
            _ => app.set_document(DiffDocument::empty("short", "fits")),
        }
        render(&mut app, &mut terminal);
        assert!(app.scrollbar_drag.is_none());
        assert!(
            mouse(
                &mut app,
                MouseEventKind::Up(MouseButton::Left),
                bar.track.right(),
                0
            )
            .is_empty()
        );
        assert_eq!(app.horizontal_scroll, 0);
        assert_eq!(app.content_scroll, 0);
    }
}

#[test]
fn vertical_and_horizontal_scrollbars_use_independent_axes_with_one_drag_state() {
    let mut app = wide_app(DiffLayout::Unified);
    let mut terminal = Terminal::new(TestBackend::new(160, 32)).unwrap();
    render(&mut app, &mut terminal);
    let horizontal = app.geometry.horizontal_scrollbar.unwrap();
    drop(mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        horizontal.track.right() - 1,
        horizontal.track.y,
    ));
    render(&mut app, &mut terminal);
    assert_eq!(app.horizontal_scroll, horizontal.max_offset);
    let vertical = app.geometry.content_scrollbar.unwrap();
    drop(mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        vertical.thumb.x,
        vertical.thumb.y,
    ));
    drop(mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        0,
        u16::MAX,
    ));
    render(&mut app, &mut terminal);
    assert_eq!(app.content_scroll, vertical.max_offset);
    assert_eq!(app.horizontal_scroll, horizontal.max_offset);
    assert!(mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 0, u16::MAX).is_empty());
    assert!(app.scrollbar_drag.is_none());
    assert!(app.text_selection.is_none());
}
