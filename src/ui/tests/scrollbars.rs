use std::time::Instant;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;

pub(super) fn render(app: &mut App, terminal: &mut Terminal<TestBackend>) {
    terminal
        .draw(|frame| draw(frame, app, &Theme::default()))
        .unwrap();
}

pub(super) fn mouse(
    app: &mut App,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> Vec<crate::app::AppEffect> {
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

#[test]
fn history_scrollbar_counts_two_line_commits_and_loads_more_at_the_end() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::History;
    app.history = (0..20)
        .map(|index| crate::git::history::Commit {
            id: format!("{index:040x}"),
            short_id: format!("{index:07x}"),
            parent_ids: Vec::new(),
            author: String::new(),
            author_email: String::new(),
            authored_at: String::new(),
            committer: String::new(),
            committer_email: String::new(),
            committed_at: String::new(),
            relative_date: "now".to_owned(),
            subject: format!("Commit {index}"),
            decorations: Vec::new(),
        })
        .collect();
    let mut terminal = Terminal::new(TestBackend::new(100, 32)).unwrap();
    render(&mut app, &mut terminal);
    let scrollbar = app.geometry.sidebar_scrollbar.unwrap();
    assert_eq!(
        scrollbar.viewport,
        usize::from(scrollbar.track.height).div_euclid(2)
    );
    assert_eq!(scrollbar.max_offset, 20 - scrollbar.viewport);
    drop(mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        scrollbar.thumb.x,
        scrollbar.thumb.y,
    ));
    let effects = mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        scrollbar.track.x,
        scrollbar.track.bottom() - 1,
    );
    render(&mut app, &mut terminal);
    assert_eq!(app.sidebar_offset, scrollbar.max_offset);
    assert_eq!(app.history_cursor, 0);
    assert!(rendered_text(terminal.backend().buffer()).contains("Commit 19"));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        crate::app::AppEffect::Git(command) if matches!(command.as_ref(),
            crate::git::worker::WorkerCommand::LoadHistory { skip: 20, .. })
    )));
}

#[test]
fn keyboard_modal_and_small_terminal_changes_cancel_scrollbar_drags() {
    for transition in 0..3 {
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
        match transition {
            0 => {
                drop(app.handle_key(
                    crossterm::event::KeyEvent::new(
                        crossterm::event::KeyCode::Char('2'),
                        KeyModifiers::NONE,
                    ),
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
            _ => terminal.backend_mut().resize(60, 12),
        }
        render(&mut app, &mut terminal);
        drop(mouse(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            scrollbar.track.x,
            scrollbar.track.bottom(),
        ));
        assert!(app.scrollbar_drag.is_none());
        assert_eq!(app.content_scroll, 0);
    }
}

#[test]
fn resizing_during_a_drag_uses_the_current_scroll_range() {
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
    terminal.backend_mut().resize(140, 40);
    render(&mut app, &mut terminal);
    let resized = app.geometry.content_scrollbar.unwrap();
    assert_ne!(resized.max_offset, scrollbar.max_offset);
    drop(mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        scrollbar.track.x,
        resized.track.bottom(),
    ));
    render(&mut app, &mut terminal);
    assert_eq!(app.content_scroll, resized.max_offset);
}
