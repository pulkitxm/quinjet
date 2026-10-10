use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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
        assert!(app.geometry.horizontal_scrollbar.is_some());
        assert!(!rendered_text(terminal.backend().buffer()).contains("[0 Reset]"));
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
    assert!(app.geometry.horizontal_scrollbar.is_none());
    assert!(rendered_text(terminal.backend().buffer()).contains("visible code"));
}

#[test]
fn horizontal_scrollbar_thumb_tracks_the_pan_and_keeps_bottom_navigation_clear() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = long_code_app(layout);
        app.document.lines.extend(std::iter::repeat_n(
            test_line(DiffLineKind::Context, "short code"),
            100,
        ));
        let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let bar = app.geometry.horizontal_scrollbar.unwrap();
        assert_eq!(bar.thumb.x, bar.area.x + 1);
        assert_eq!(bar.area.y, app.geometry.content.bottom() - 1);
        let bottom = app
            .geometry
            .scm_action_hits
            .iter()
            .find(|hit| hit.action == ScmAction::JumpToBottom)
            .unwrap();
        assert!(bar.area.right() <= bottom.area.x);
        let text = rendered_text(terminal.backend().buffer());
        assert!(!text.contains("←/→"));
        assert!(!text.contains("[0 Reset]"));
        assert_eq!(
            terminal.backend().buffer()[(bar.area.x, bar.area.y)].symbol(),
            "◀"
        );
        assert_eq!(
            terminal.backend().buffer()[(bar.thumb.x, bar.thumb.y)].symbol(),
            "━"
        );
        app.horizontal_scroll = app.horizontal_scroll_limit;
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let end = app.geometry.horizontal_scrollbar.unwrap();
        assert_eq!(end.thumb.right(), end.area.right() - 1);
    }
}

#[test]
fn horizontal_scrollbar_accepts_track_clicks_arrows_and_bounded_thumb_dragging() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = long_code_app(layout);
        let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let bar = app.geometry.horizontal_scrollbar.unwrap();
        swipe(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            bar.area.right() - 1,
            bar.area.y,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, 3);
        swipe(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            bar.area.x,
            bar.area.y,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, 0);
        swipe(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            bar.area.right() - 2,
            bar.area.y,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, app.horizontal_scroll_limit);
        assert!(app.text_selection.is_none());
        swipe(
            &mut app,
            MouseEventKind::Up(MouseButton::Left),
            bar.area.right() - 2,
            bar.area.y,
            KeyModifiers::NONE,
        );
        app.horizontal_scroll /= 2;
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let middle = app.geometry.horizontal_scrollbar.unwrap();
        let offset = app.horizontal_scroll;
        swipe(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            middle.thumb.x + 1,
            middle.thumb.y,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, offset);
        swipe(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            middle.thumb.x + 1,
            middle.thumb.y,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, offset);
        swipe(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            middle.area.right() + 100,
            middle.area.y + 2,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, app.horizontal_scroll_limit);
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        swipe(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            0,
            middle.area.y + 2,
            KeyModifiers::NONE,
        );
        assert_eq!(app.horizontal_scroll, 0);
        let effects = app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                column: 0,
                row: middle.area.y + 2,
                modifiers: KeyModifiers::NONE,
            },
            Instant::now(),
        );
        assert!(effects.is_empty());
        assert!(app.horizontal_scroll_drag.is_none());
        assert!(app.text_selection.is_none());
    }
}

#[test]
fn horizontal_scrollbar_reflows_on_resize_and_disappears_when_code_wraps() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = long_code_app(layout);
        let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let wide = app.geometry.horizontal_scrollbar.unwrap();
        let mut narrow = Terminal::new(TestBackend::new(120, 24)).unwrap();
        narrow
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let bar = app.geometry.horizontal_scrollbar.unwrap();
        assert!(bar.area.width < wide.area.width);
        swipe(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            bar.thumb.x,
            bar.thumb.y,
            KeyModifiers::NONE,
        );
        assert!(app.horizontal_scroll_drag.is_some());
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT),
            Instant::now(),
        );
        narrow
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        assert_eq!(app.horizontal_scroll, 0);
        assert!(app.geometry.horizontal_scrollbar.is_none());
        assert!(app.horizontal_scroll_drag.is_none());
    }
}
