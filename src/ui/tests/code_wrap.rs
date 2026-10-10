use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;

fn wrap_app(layout: DiffLayout) -> App {
    let mut app = App::new("/example/project", "example");
    app.diff_layout = layout;
    app.focus = Focus::Content;
    let mut code = test_line(
        DiffLineKind::Added,
        &format!("{}END_OF_LINE", "界0123456789".repeat(20)),
    );
    code.new_line = Some(42);
    app.set_document(DiffDocument {
        lines: vec![
            test_file_header("example.rs", 1, 0),
            code,
            test_line(DiffLineKind::FileFooter, ""),
        ],
        ..DiffDocument::default()
    });
    app
}

#[test]
fn wrap_shortcut_reveals_the_tail_and_preserves_line_numbers_in_both_layouts() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = wrap_app(layout);
        let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        assert!(!rendered_text(terminal.backend().buffer()).contains("END_OF_LINE"));
        app.horizontal_scroll = 20;
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT),
            Instant::now(),
        );
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let text = rendered_text(terminal.backend().buffer());
        assert!(app.wrap_code);
        let code = visible_wrapped_code(terminal.backend().buffer(), &app, layout);
        assert!(code.ends_with("END_OF_LINE"), "{layout:?}: {code}");
        assert_eq!(text.matches("  42 ").count(), 1);
        assert_eq!(text.matches('界').count(), 20);
        assert_eq!(app.horizontal_scroll, 0);
        assert_eq!(app.horizontal_scroll_regions.len(), 0);
        let pointer = app.diff_wrap_offsets.as_ptr();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        assert_eq!(app.diff_wrap_offsets.as_ptr(), pointer);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT),
            Instant::now(),
        );
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        assert!(!app.wrap_code);
        assert!(!rendered_text(terminal.backend().buffer()).contains("END_OF_LINE"));
        assert!(app.horizontal_scroll_limit > 0);
    }
}

#[test]
fn wrapping_uses_each_side_width_and_reflows_when_the_divider_moves() {
    let mut app = wrap_app(DiffLayout::SideBySide);
    app.wrap_code = true;
    app.diff_split_percent = 30;
    app.document
        .lines
        .insert(1, test_line(DiffLineKind::Removed, &"old text ".repeat(30)));
    app.invalidate_diff_rows();
    let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let narrow_old_rows = app
        .side_by_side_diff_rows
        .iter()
        .filter(|row| matches!(row, SideBySideRow::Split(Some(1), _)))
        .count();
    assert!(
        app.side_by_side_diff_rows
            .iter()
            .any(|row| matches!(row, SideBySideRow::Split(Some(1), None)))
    );
    app.diff_split_percent = 70;
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let wide_old_rows = app
        .side_by_side_diff_rows
        .iter()
        .filter(|row| matches!(row, SideBySideRow::Split(Some(1), _)))
        .count();
    assert!(wide_old_rows < narrow_old_rows);
    assert!(rendered_text(terminal.backend().buffer()).contains("END_OF_LINE"));
}

#[test]
fn wrap_toggling_keeps_the_same_source_line_at_the_top() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = wrap_app(layout);
        app.document
            .lines
            .extend((0..100).map(|row| test_line(DiffLineKind::Context, &format!("line {row}"))));
        app.invalidate_diff_rows();
        app.content_scroll = 15;
        let mut terminal = Terminal::new(TestBackend::new(180, 30)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let index = app.diff_scroll_anchor.unwrap().0;
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT),
            Instant::now(),
        );
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        assert_eq!(app.diff_scroll_anchor.unwrap().0, index);
        assert!(app.content_scroll > 15);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT),
            Instant::now(),
        );
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        assert_eq!(app.diff_scroll_anchor.unwrap().0, index);
        assert_eq!(app.content_scroll, 15);
    }
}

#[test]
fn clipping_a_wide_character_does_not_pull_later_highlight_spans_forward() {
    let spans = vec![
        HighlightSpan {
            text: "abc界".to_owned(),
            foreground: None,
            bold: false,
            italic: false,
        },
        HighlightSpan {
            text: "tail".to_owned(),
            foreground: None,
            bold: true,
            italic: false,
        },
    ];
    let first = highlight_spans(&spans, 0, 4, DiffLineKind::Added, None, &Theme::default());
    assert_eq!(
        first
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>(),
        "abc"
    );
    let second = highlight_spans(&spans, 3, 6, DiffLineKind::Added, None, &Theme::default());
    assert_eq!(
        second
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>(),
        "界tail"
    );
    assert!(
        second
            .last()
            .unwrap()
            .style
            .add_modifier
            .contains(Modifier::BOLD)
    );
}

#[expect(clippy::integer_division, reason = "the test uses equal panes")]
fn visible_wrapped_code(buffer: &Buffer, app: &App, layout: DiffLayout) -> String {
    let area = app.geometry.content;
    let width = area.width.saturating_sub(1);
    let (start, end) = if layout == DiffLayout::SideBySide {
        (
            area.x + width.saturating_sub(1) / 2 + 1 + 7,
            area.right() - 1,
        )
    } else {
        (area.x + 12, area.right() - 1)
    };
    let mut result = String::new();
    let rows = if layout == DiffLayout::SideBySide {
        &app.side_by_side_diff_rows
            .iter()
            .enumerate()
            .filter_map(|(row, value)| {
                matches!(value, SideBySideRow::Split(_, Some(1))).then_some(row)
            })
            .collect::<Vec<_>>()
    } else {
        &app.unified_diff_rows
            .iter()
            .enumerate()
            .filter_map(|(row, index)| (*index == 1).then_some(row))
            .collect::<Vec<_>>()
    };
    for row in rows {
        let mut column = start;
        let mut text = String::new();
        while column < end {
            let symbol = buffer[(column, area.y + 1 + cells(*row))].symbol();
            text.push_str(symbol);
            column += cells(symbol.width().max(1));
        }
        result.push_str(text.trim_end());
    }
    result
}
