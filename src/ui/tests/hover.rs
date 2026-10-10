use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;

fn hover_at(app: &mut App, column: u16, row: u16) {
    drop(app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Moved,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    ));
}

fn diff_app(layout: DiffLayout, wrap: bool) -> App {
    let mut app = App::new("/example/project", "example");
    app.sidebar_hidden = true;
    app.diff_layout = layout;
    app.wrap_code = wrap;
    app.set_document(DiffDocument {
        lines: vec![
            test_file_header("example.rs", 1, 1),
            test_line(DiffLineKind::Removed, &"old 界 text ".repeat(8)),
            test_line(DiffLineKind::Added, &"new 界 text ".repeat(24)),
            test_line(DiffLineKind::Context, "untouched"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
        ..DiffDocument::default()
    });
    app
}

#[test]
fn diff_hover_tracks_pairs_and_every_wrapped_continuation() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        for wrap in [false, true] {
            for appearance in [
                crate::theme::Appearance::Dark,
                crate::theme::Appearance::Light,
            ] {
                let theme = Theme::new(ThemeName::Quinjet, appearance);
                let mut app = diff_app(layout, wrap);
                let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
                terminal
                    .draw(|frame| draw(frame, &mut app, &theme))
                    .unwrap();
                let viewport = app.geometry.diff_viewport.unwrap();
                let index = if layout == DiffLayout::Unified { 2 } else { 1 };
                let row = if wrap { index + 1 } else { index };
                let column = viewport.area.right() - 4;
                let before = terminal.backend().buffer().clone();
                hover_at(&mut app, column, viewport.area.y + row);
                terminal
                    .draw(|frame| draw(frame, &mut app, &theme))
                    .unwrap();
                let after = terminal.backend().buffer();
                let mut tinted_rows = 0;
                for y in viewport.area.y..viewport.area.bottom() {
                    if after[(column, y)].bg != before[(column, y)].bg {
                        tinted_rows += 1;
                    }
                }
                assert!(
                    tinted_rows >= if wrap { 3 } else { 1 },
                    "{layout:?} wrap={wrap}"
                );
                for (before, after) in before.content().iter().zip(after.content()) {
                    assert_eq!(before.symbol(), after.symbol());
                    assert_eq!(before.fg, after.fg);
                }
                if layout == DiffLayout::SideBySide {
                    let y = viewport.area.y + row;
                    assert_ne!(
                        before[(viewport.area.x + 2, y)].bg,
                        after[(viewport.area.x + 2, y)].bg
                    );
                }
                let context_y = (viewport.area.y..viewport.area.bottom())
                    .find(|y| {
                        (viewport.area.x..viewport.area.right())
                            .any(|x| after[(x, *y)].symbol() == "u")
                    })
                    .unwrap();
                assert_eq!(
                    before[(column, context_y)].bg,
                    after[(column, context_y)].bg
                );
            }
        }
    }
}

#[test]
fn hover_uses_the_scrolled_viewport_and_reflows_on_resize() {
    let theme = Theme::default();
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = diff_app(layout, true);
        app.document.lines.splice(
            3..3,
            std::iter::repeat_n(test_line(DiffLineKind::Context, "other"), 100),
        );
        app.content_scroll = 2;
        let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &theme))
            .unwrap();
        let viewport = app.geometry.diff_viewport.unwrap();
        let before = terminal.backend().buffer().clone();
        hover_at(&mut app, viewport.area.x + 15, viewport.area.y);
        terminal
            .draw(|frame| draw(frame, &mut app, &theme))
            .unwrap();
        assert_ne!(
            before[(viewport.area.x + 15, viewport.area.y)].bg,
            terminal.backend().buffer()[(viewport.area.x + 15, viewport.area.y)].bg
        );
        app.sidebar_hidden = false;
        let mut narrow = Terminal::new(TestBackend::new(80, 24)).unwrap();
        narrow.draw(|frame| draw(frame, &mut app, &theme)).unwrap();
        assert!(!app.geometry.diff_viewport.unwrap().side_by_side);
        assert!(rendered_text(narrow.backend().buffer()).contains("example.rs"));
    }
}

#[test]
fn conversation_hover_does_not_change_navigation_or_links() {
    let mut app = overview_app();
    app.pull_request_section = PullRequestSection::Overview;
    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    let before = terminal.backend().buffer().clone();
    let content = app.geometry.content;
    let y = content.y + 3;
    let links = app.geometry.link_hits.len();
    let focus = app.focus;
    hover_at(&mut app, content.x + 10, y);
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    assert_ne!(
        before[(content.x + 10, y)].bg,
        terminal.backend().buffer()[(content.x + 10, y)].bg
    );
    assert_eq!(app.focus, focus);
    assert_eq!(app.content_scroll, 0);
    assert_eq!(app.geometry.link_hits.len(), links);
    assert!(app.geometry.diff_viewport.is_none());
    drop(app.handle_key(
        KeyEvent::new(KeyCode::Null, KeyModifiers::NONE),
        Instant::now(),
    ));
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    assert_eq!(before, *terminal.backend().buffer());
}

#[test]
fn hovering_a_commit_highlights_both_rows_without_selecting_it() {
    let mut app = App::new("/example/project", "example");
    app.view = View::History;
    app.history = (0..3)
        .map(|index| crate::git::history::Commit {
            id: format!("{index:040x}"),
            short_id: format!("{index:07x}"),
            parent_ids: Vec::new(),
            author: "Example".to_owned(),
            author_email: "example@example.invalid".to_owned(),
            authored_at: String::new(),
            committer: "Example".to_owned(),
            committer_email: "example@example.invalid".to_owned(),
            committed_at: String::new(),
            relative_date: "now".to_owned(),
            subject: format!("example commit {index}"),
            decorations: Vec::new(),
        })
        .collect();
    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    let hit = app
        .geometry
        .sidebar_hits
        .iter()
        .find(|hit| matches!(hit.target, SidebarHit::Commit(1)))
        .unwrap()
        .area;
    let before = terminal.backend().buffer().clone();
    hover_at(&mut app, hit.x + 10, hit.y + 1);
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    assert_eq!(app.history_cursor, 0);
    assert!(!app.document_loading);
    for y in hit.y..hit.bottom() {
        assert_ne!(
            before[(hit.x + 10, y)].bg,
            terminal.backend().buffer()[(hit.x + 10, y)].bg
        );
    }
}

#[test]
fn selection_and_modals_take_precedence_over_hover() {
    let theme = Theme::default();
    let mut app = diff_app(DiffLayout::Unified, false);
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    let viewport = app.geometry.diff_viewport.unwrap();
    let point = (viewport.area.x + 12, viewport.area.y + 2);
    hover_at(&mut app, point.0, point.1);
    app.text_selection = Some(crate::app::TextSelection {
        pane: viewport.area,
        anchor: point,
        head: (point.0 + 5, point.1),
    });
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    assert_eq!(terminal.backend().buffer()[point].bg, theme.selected);
    app.text_selection = None;
    app.modal = Some(Modal::Help {
        selected: 0,
        scroll: 0,
        hover: None,
    });
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    let hovered = terminal.backend().buffer().clone();
    app.mouse_hover = None;
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    assert_eq!(hovered, *terminal.backend().buffer());
}

#[test]
fn scrollbar_drag_takes_precedence_over_hover_feedback() {
    use super::scrollbars::mouse;

    let theme = Theme::default();
    let mut app = diff_app(DiffLayout::Unified, false);
    app.document.lines.splice(
        3..3,
        std::iter::repeat_n(test_line(DiffLineKind::Context, "example text"), 100),
    );
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    let scrollbar = app.geometry.content_scrollbar.unwrap();
    let viewport = app.geometry.diff_viewport.unwrap();
    hover_at(&mut app, viewport.area.x + 15, viewport.area.y + 2);
    app.link_hover = app.mouse_hover;
    drop(mouse(
        &mut app,
        MouseEventKind::Down(crossterm::event::MouseButton::Left),
        scrollbar.thumb.x,
        scrollbar.thumb.y,
    ));
    assert!(app.scrollbar_drag.is_some());
    assert!(app.mouse_hover.is_none());
    assert!(app.link_hover.is_none());
    hover_at(&mut app, viewport.area.x + 15, viewport.area.y + 2);
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    let dragging = terminal.backend().buffer().clone();
    app.mouse_hover = None;
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    assert_eq!(dragging, *terminal.backend().buffer());
    let effects = mouse(
        &mut app,
        MouseEventKind::Up(crossterm::event::MouseButton::Left),
        scrollbar.thumb.x,
        scrollbar.thumb.y,
    );
    assert!(effects.is_empty());
    assert!(app.scrollbar_drag.is_none());
}

#[test]
fn scrollbar_click_remains_available_after_momentum_crosses_panes() {
    use super::scrollbars::mouse;

    let theme = Theme::default();
    let mut app = diff_app(DiffLayout::Unified, false);
    app.sidebar_hidden = false;
    app.document.lines.splice(
        3..3,
        std::iter::repeat_n(test_line(DiffLineKind::Context, "example text"), 100),
    );
    app.status.changes = (0..100)
        .map(|index| Change {
            path: format!("example-{index}.txt").into(),
            original_path: None,
            area: ChangeArea::Unstaged,
            status: ChangeStatus::Modified,
        })
        .collect();
    let mut terminal = Terminal::new(TestBackend::new(160, 32)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    let content = app.geometry.content;
    let sidebar = app.geometry.sidebar;
    drop(mouse(
        &mut app,
        MouseEventKind::ScrollDown,
        content.x + 10,
        content.y + 5,
    ));
    hover_at(&mut app, sidebar.x + 10, sidebar.y + 5);
    drop(mouse(
        &mut app,
        MouseEventKind::ScrollDown,
        sidebar.x + 10,
        sidebar.y + 5,
    ));
    assert_eq!(app.sidebar_offset, 0);
    let scrollbar = app.geometry.sidebar_scrollbar.unwrap();
    drop(mouse(
        &mut app,
        MouseEventKind::Down(crossterm::event::MouseButton::Left),
        scrollbar.thumb.x,
        scrollbar.thumb.y,
    ));
    drop(mouse(
        &mut app,
        MouseEventKind::Drag(crossterm::event::MouseButton::Left),
        scrollbar.thumb.x,
        scrollbar.track.y + 10,
    ));
    assert!(app.sidebar_offset > 0);
    let effects = mouse(
        &mut app,
        MouseEventKind::Up(crossterm::event::MouseButton::Left),
        scrollbar.thumb.x,
        scrollbar.track.y + 10,
    );
    assert!(effects.is_empty());
    let offset = app.sidebar_offset;
    drop(mouse(
        &mut app,
        MouseEventKind::ScrollUp,
        sidebar.x + 10,
        sidebar.y + 5,
    ));
    assert_eq!(app.sidebar_offset, offset - 2);
}
