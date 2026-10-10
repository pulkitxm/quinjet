use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;

fn long_code_app(layout: DiffLayout) -> App {
    let mut app = App::new("/example/project", "example");
    app.diff_layout = layout;
    app.focus = Focus::Content;
    app.set_document(DiffDocument {
        lines: vec![
            test_file_header("example.rs", 1, 0),
            test_line(DiffLineKind::Added, &"界0123456789".repeat(30)),
            test_line(DiffLineKind::FileFooter, ""),
        ],
        ..DiffDocument::default()
    });
    app
}

fn swipe(app: &mut App, kind: MouseEventKind, column: u16, row: u16, modifiers: KeyModifiers) {
    app.handle_mouse(
        MouseEvent {
            kind,
            column,
            row,
            modifiers,
        },
        Instant::now(),
    );
}

#[test]
fn horizontal_wheel_events_outside_code_never_move_the_preview() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = long_code_app(layout);
        let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        for (column, row) in [(4, 1), (4, 10), (90, 3), (90, 4), (90, 46), (90, 30)] {
            swipe(
                &mut app,
                MouseEventKind::ScrollRight,
                column,
                row,
                KeyModifiers::NONE,
            );
            swipe(
                &mut app,
                MouseEventKind::ScrollDown,
                column,
                row,
                KeyModifiers::SHIFT,
            );
            assert_eq!(app.horizontal_scroll, 0);
        }
        let region = app.horizontal_scroll_regions[0];
        swipe(
            &mut app,
            MouseEventKind::ScrollRight,
            region.x - 1,
            region.y,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, 0);
        if layout == DiffLayout::SideBySide {
            let column = app.geometry.content.x + 3;
            swipe(
                &mut app,
                MouseEventKind::ScrollRight,
                column,
                region.y,
                KeyModifiers::NONE,
            );
            assert_eq!(app.horizontal_scroll, 0);
        }
        swipe(
            &mut app,
            MouseEventKind::ScrollRight,
            region.x,
            region.y,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, 3);
    }
}

#[test]
fn horizontal_swipes_over_short_code_rows_pan_the_overflowing_diff() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = long_code_app(layout);
        app.document
            .lines
            .insert(2, test_line(DiffLineKind::Added, "short code"));
        let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let region = app.horizontal_scroll_regions[0];
        swipe(
            &mut app,
            MouseEventKind::ScrollRight,
            region.x,
            region.y + 1,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, 3);
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        swipe(
            &mut app,
            MouseEventKind::ScrollLeft,
            region.x,
            region.y + 1,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, 0);
    }
}

#[test]
fn repeated_swipes_stop_at_the_visible_text_extent_and_reset_is_reachable() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = long_code_app(layout);
        let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let region = app.horizontal_scroll_regions[0];
        for _ in 0..1_000 {
            swipe(
                &mut app,
                MouseEventKind::ScrollRight,
                region.x,
                region.y,
                KeyModifiers::NONE,
            );
        }
        assert_eq!(app.horizontal_scroll, app.horizontal_scroll_limit);
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        assert!(rendered_text(terminal.backend().buffer()).contains("[0 Reset]"));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
            Instant::now(),
        );
        assert_eq!(app.horizontal_scroll, 0);
        for _ in 0..1_000 {
            swipe(
                &mut app,
                MouseEventKind::ScrollLeft,
                region.x,
                region.y,
                KeyModifiers::NONE,
            );
        }
        assert_eq!(app.horizontal_scroll, 0);
    }
}

#[test]
fn shorter_content_and_wider_panes_clamp_an_old_pan_offset() {
    let mut app = long_code_app(DiffLayout::Unified);
    let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
    app.horizontal_scroll = usize::MAX;
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_eq!(app.horizontal_scroll, app.horizontal_scroll_limit);
    app.set_document(DiffDocument::empty("short", "visible code"));
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_eq!(app.horizontal_scroll, 0);
    assert_eq!(app.horizontal_scroll_regions.len(), 0);
    assert!(rendered_text(terminal.backend().buffer()).contains("visible code"));
}
