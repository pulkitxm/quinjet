mod theme;

use std::collections::HashMap;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{
    App, DiffLayout, Focus, Modal, PaletteCommand, SidebarHit, ToastLevel, UiGeometry, View,
};
use crate::git::Branch;
use crate::git::diff::{DiffDocument, DiffLine, DiffLineKind, HighlightSpan};
use crate::git::status::{Change, ChangeArea, ChangeStatus};

use self::theme::Theme;

const HELP_LINES: &[(&str, &str)] = &[
    ("Navigation", ""),
    ("j / k, ↑ / ↓", "Move selection or scroll preview"),
    ("PgUp / PgDn", "Move by a page"),
    ("gg / G", "Jump to first / last item"),
    ("Tab", "Switch focus between sidebar and preview"),
    ("Enter", "Focus preview"),
    ("h / l, ← / →", "Scroll preview horizontally"),
    ("[ / ]", "Previous / next diff hunk"),
    ("1 / 2", "Changes / commit history"),
    ("/", "Filter the active list"),
    ("Esc", "Clear filter, close modal, or return focus"),
    ("", ""),
    ("Changes", ""),
    ("s / u", "Stage / unstage selected file"),
    ("a / U", "Stage all / unstage all"),
    ("x", "Discard selected change (asks first)"),
    ("c", "Commit staged changes"),
    ("b", "Switch, create, or delete branches"),
    ("", ""),
    ("History", ""),
    ("C / R", "Cherry-pick / revert selected commit"),
    ("n", "Create branch at selected commit"),
    ("", ""),
    ("Repository", ""),
    ("r / Ctrl+R", "Refresh"),
    ("f / l / p / y", "Fetch / pull / push / sync"),
    ("v", "Toggle unified / side-by-side diff"),
    (": / Ctrl+P", "Open command palette"),
    ("?", "Show this help"),
    ("q", "Quit"),
];

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let theme = Theme::default();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.background)),
        frame.area(),
    );

    if frame.area().width < 72 || frame.area().height < 18 {
        draw_too_small(frame, &theme);
        return;
    }

    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(2),
    ])
    .split(frame.area());
    let tabs = vertical[0];
    let main = vertical[1];
    let footer = vertical[2];
    let sidebar_width = ((main.width as f32 * 0.34) as u16).clamp(30, 52);
    let columns =
        Layout::horizontal([Constraint::Length(sidebar_width), Constraint::Min(40)]).split(main);

    let (changes_tab, history_tab) = draw_tabs(frame, tabs, app, &theme);
    let sidebar_hits = draw_sidebar(frame, columns[0], app, &theme);
    draw_content(frame, columns[1], app, &theme);
    draw_footer(frame, footer, app, &theme);

    app.geometry = UiGeometry {
        changes_tab,
        history_tab,
        sidebar: columns[0],
        content: columns[1],
        sidebar_hits,
    };

    if let Some(modal) = app.modal.as_ref() {
        draw_modal(frame, modal, app, &theme);
    }
    if let Some(toast) = app.toast.as_ref() {
        draw_toast(frame, toast.message.as_str(), toast.level, &theme);
    }
}

fn draw_too_small(frame: &mut Frame<'_>, theme: &Theme) {
    let text = Text::from(vec![
        Line::from(Span::styled(
            "Quinjet",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Terminal too small",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            "Resize to at least 72 × 18",
            Style::default().fg(theme.muted),
        )),
    ]);
    frame.render_widget(
        Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border)),
        ),
        centered_rect(50, 8, frame.area()),
    );
}

fn draw_tabs(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) -> (Rect, Rect) {
    let repository = if app.repository_root.to_string_lossy() == app.repository_name {
        app.repository_name.clone()
    } else {
        format!("{}  {}", app.repository_name, app.repository_root.display())
    };
    let branch = if app.status.branch.head.is_empty() {
        "detecting branch…".to_owned()
    } else {
        let mut text = format!(" {}", app.status.branch.head);
        if app.status.branch.ahead > 0 {
            text.push_str(&format!("  ↑{}", app.status.branch.ahead));
        }
        if app.status.branch.behind > 0 {
            text.push_str(&format!("  ↓{}", app.status.branch.behind));
        }
        text
    };
    let header = Layout::horizontal([
        Constraint::Length(16),
        Constraint::Length(16),
        Constraint::Min(8),
        Constraint::Length((branch.width() + 3).min(area.width as usize) as u16),
    ])
    .split(area);

    draw_tab(
        frame,
        header[0],
        "  Changes  [1]",
        app.view == View::Changes,
        theme,
    );
    draw_tab(
        frame,
        header[1],
        "  History  [2]",
        app.view == View::History,
        theme,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " QUINJET ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(repository, Style::default().fg(theme.text)),
        ]))
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.panel)),
        ),
        header[2],
    );
    frame.render_widget(
        Paragraph::new(branch)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.accent).bg(theme.panel))
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(Style::default().fg(theme.border)),
            ),
        header[3],
    );
    (header[0], header[1])
}

fn draw_tab(frame: &mut Frame<'_>, area: Rect, label: &str, active: bool, theme: &Theme) {
    let style = if active {
        Style::default()
            .fg(theme.text)
            .bg(theme.accent_soft)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted).bg(theme.panel)
    };
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Center)
            .style(style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if active {
                        theme.border_focus
                    } else {
                        theme.border
                    })),
            ),
        area,
    );
}

fn draw_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<(u16, SidebarHit)> {
    match app.view {
        View::Changes => draw_changes_sidebar(frame, area, app, theme),
        View::History => draw_history_sidebar(frame, area, app, theme),
    }
}

fn draw_changes_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<(u16, SidebarHit)> {
    let block = panel_block(
        if app.filter.is_empty() {
            format!(" Source Control  {} ", app.status.changes.len())
        } else {
            format!(" Source Control  /{} ", app.filter)
        },
        app.focus == Focus::Sidebar && app.modal.is_none(),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return Vec::new();
    }

    let commit_height = 3.min(inner.height);
    let regions =
        Layout::vertical([Constraint::Length(commit_height), Constraint::Min(1)]).split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" Commit message", Style::default().fg(theme.muted)),
                Span::styled("  [c]", Style::default().fg(theme.accent)),
            ]),
            Line::from(Span::styled(
                " Press c, type message, Ctrl+Enter",
                Style::default().fg(theme.muted),
            )),
        ])
        .style(Style::default().bg(theme.panel_alt))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border)),
        ),
        regions[0],
    );

    let visible = app.visible_change_indices();
    let row_count = change_row_count(app, &visible);
    let height = regions[1].height as usize;
    ensure_offset(
        &mut app.sidebar_offset,
        app.change_cursor,
        height,
        row_count,
    );
    let rows = build_change_rows(app, &visible);
    let mut hits = vec![(regions[0].y, SidebarHit::CommitInput)];
    let end = (app.sidebar_offset + height).min(rows.len());
    for (y, row) in
        (regions[1].y..regions[1].bottom()).zip(rows.iter().take(end).skip(app.sidebar_offset))
    {
        match row {
            ChangeRow::Header { area: group, count } => {
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(" ▾ ", Style::default().fg(theme.muted)),
                        Span::styled(
                            group.label().to_uppercase(),
                            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("  {count}"), Style::default().fg(theme.muted)),
                    ]))
                    .style(Style::default().bg(theme.panel_alt)),
                    Rect::new(regions[1].x, y, regions[1].width, 1),
                );
            }
            ChangeRow::Change {
                index,
                cursor,
                change,
            } => {
                let selected = *cursor == app.change_cursor;
                let row_style = if selected {
                    Style::default().bg(theme.selected)
                } else {
                    Style::default().bg(theme.panel)
                };
                let path = change.parent_path();
                let available = regions[1].width.saturating_sub(8) as usize;
                let name = truncate_middle(
                    &change.file_name(),
                    available.saturating_sub(path.width() + 1),
                );
                let line = Line::from(vec![
                    Span::styled(
                        if selected { " › " } else { "   " },
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(
                        name,
                        Style::default().fg(theme.text).add_modifier(if selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    ),
                    Span::styled(
                        if path.is_empty() {
                            String::new()
                        } else {
                            format!("  {path}")
                        },
                        Style::default().fg(theme.muted),
                    ),
                ]);
                frame.render_widget(
                    Paragraph::new(line).style(row_style),
                    Rect::new(regions[1].x, y, regions[1].width.saturating_sub(4), 1),
                );
                let badge = format!(" {} ", change.status.code());
                frame.render_widget(
                    Paragraph::new(badge).alignment(Alignment::Right).style(
                        Style::default()
                            .fg(status_color(change.status, theme))
                            .bg(if selected {
                                theme.selected
                            } else {
                                theme.panel
                            })
                            .add_modifier(Modifier::BOLD),
                    ),
                    Rect::new(regions[1].right().saturating_sub(4), y, 4, 1),
                );
                hits.push((y, SidebarHit::Change(*index)));
            }
        }
    }

    if visible.is_empty() {
        let message = if app.status.changes.is_empty() {
            "\n  ✓ Working tree clean\n\n  No pending changes"
        } else {
            "\n  No changes match this filter"
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(Style::default().fg(if app.status.changes.is_empty() {
                    theme.success
                } else {
                    theme.muted
                }))
                .wrap(Wrap { trim: false }),
            regions[1],
        );
    }
    hits
}

#[derive(Clone)]
enum ChangeRow<'a> {
    Header {
        area: ChangeArea,
        count: usize,
    },
    Change {
        index: usize,
        cursor: usize,
        change: &'a Change,
    },
}

fn change_row_count(app: &App, visible: &[usize]) -> usize {
    let group_count = [
        ChangeArea::Conflict,
        ChangeArea::Staged,
        ChangeArea::Unstaged,
    ]
    .into_iter()
    .filter(|area| {
        visible
            .iter()
            .any(|index| app.status.changes[*index].area == *area)
    })
    .count();
    visible.len() + group_count
}

fn build_change_rows<'a>(app: &'a App, visible: &[usize]) -> Vec<ChangeRow<'a>> {
    let mut rows = Vec::new();
    let mut cursor_map = HashMap::new();
    for (cursor, index) in visible.iter().enumerate() {
        cursor_map.insert(*index, cursor);
    }
    for area in [
        ChangeArea::Conflict,
        ChangeArea::Staged,
        ChangeArea::Unstaged,
    ] {
        let group: Vec<_> = visible
            .iter()
            .filter(|index| app.status.changes[**index].area == area)
            .copied()
            .collect();
        if group.is_empty() {
            continue;
        }
        rows.push(ChangeRow::Header {
            area,
            count: group.len(),
        });
        for index in group {
            rows.push(ChangeRow::Change {
                index,
                cursor: cursor_map[&index],
                change: &app.status.changes[index],
            });
        }
    }
    rows
}

fn draw_history_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<(u16, SidebarHit)> {
    let title = if app.filter.is_empty() {
        format!(
            " Commit History  {}{} ",
            app.history.len(),
            if app.history_complete { "" } else { "+" }
        )
    } else {
        format!(" Commit History  /{} ", app.filter)
    };
    let block = panel_block(
        title,
        app.focus == Focus::Sidebar && app.modal.is_none(),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return Vec::new();
    }

    let visible = app.visible_commit_indices();
    let height = inner.height as usize;
    ensure_offset(
        &mut app.sidebar_offset,
        app.history_cursor,
        height,
        visible.len(),
    );
    let mut hits = Vec::new();
    let end = (app.sidebar_offset + height).min(visible.len());
    for (row_offset, index) in visible
        .iter()
        .take(end)
        .skip(app.sidebar_offset)
        .enumerate()
    {
        let cursor = app.sidebar_offset + row_offset;
        let commit = &app.history[*index];
        let selected = cursor == app.history_cursor;
        let y = inner.y + row_offset as u16;
        let row_style = Style::default().bg(if selected {
            theme.selected
        } else {
            theme.panel
        });
        let graph = history_glyph(commit, cursor);
        let badge = commit
            .decorations
            .first()
            .map(|decoration| format!("  {}", clean_decoration(decoration)))
            .unwrap_or_default();
        let reserved = commit.short_id.width() + 8;
        let subject = truncate_middle(
            &commit.subject,
            (inner.width as usize).saturating_sub(reserved + badge.width()),
        );
        let line = Line::from(vec![
            Span::styled(
                if selected { " › " } else { "   " },
                Style::default().fg(theme.accent),
            ),
            Span::styled(graph, Style::default().fg(graph_color(cursor, theme))),
            Span::styled(
                subject,
                Style::default().fg(theme.text).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Span::styled(badge, Style::default().fg(theme.modified)),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(row_style),
            Rect::new(inner.x, y, inner.width.saturating_sub(10), 1),
        );
        frame.render_widget(
            Paragraph::new(commit.short_id.as_str())
                .alignment(Alignment::Right)
                .style(Style::default().fg(theme.muted).bg(if selected {
                    theme.selected
                } else {
                    theme.panel
                })),
            Rect::new(inner.right().saturating_sub(10), y, 9, 1),
        );
        hits.push((y, SidebarHit::Commit(*index)));
    }

    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.history_loading {
                "\n  Loading commit history…"
            } else if app.history.is_empty() {
                "\n  No commits yet"
            } else {
                "\n  No commits match this filter"
            })
            .style(Style::default().fg(theme.muted)),
            inner,
        );
    } else if app.history_loading && inner.height > 1 {
        let area = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
        frame.render_widget(
            Paragraph::new("  Loading more commits…")
                .style(Style::default().fg(theme.accent).bg(theme.panel_alt)),
            area,
        );
    }
    hits
}

fn draw_content(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    let title = format!(
        " {}{} ",
        truncate_middle(&app.document.title, area.width.saturating_sub(18) as usize),
        if app.document_loading { "  ⟳" } else { "" }
    );
    let block = panel_block(
        title,
        app.focus == Focus::Content && app.modal.is_none(),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let max_scroll = app
        .document
        .lines
        .len()
        .saturating_sub(inner.height as usize);
    app.content_scroll = app.content_scroll.min(max_scroll);
    match app.diff_layout {
        DiffLayout::Unified => draw_unified_diff(frame, inner, app, theme),
        DiffLayout::SideBySide if inner.width >= 92 => {
            draw_side_by_side_diff(frame, inner, app, theme);
        }
        DiffLayout::SideBySide => draw_unified_diff(frame, inner, app, theme),
    }
    draw_scrollbar(
        frame,
        inner,
        app.content_scroll,
        app.document.lines.len(),
        theme,
    );
}

fn draw_unified_diff(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let number_width = 6usize;
    let visible_width = area.width as usize;
    let lines = app
        .document
        .lines
        .iter()
        .skip(app.content_scroll)
        .take(area.height as usize)
        .map(|line| {
            let (marker, marker_style) = marker_for(line.kind, theme);
            let old = line
                .old_line
                .map_or(String::new(), |number| number.to_string());
            let new = line
                .new_line
                .map_or(String::new(), |number| number.to_string());
            let mut spans = vec![
                Span::styled(format!("{old:>4} "), Style::default().fg(theme.muted)),
                Span::styled(format!("{new:>4} "), Style::default().fg(theme.muted)),
                Span::styled(marker, marker_style),
            ];
            spans.extend(highlight_spans(
                &line.spans,
                app.horizontal_scroll,
                visible_width.saturating_sub(number_width * 2 + 2),
                line.kind,
                theme,
            ));
            Line::from(spans).style(line_background(line.kind, theme))
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_side_by_side_diff(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let columns =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    let left_area = columns[0];
    let right_area = columns[1];
    frame.render_widget(
        Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(theme.border)),
        left_area,
    );

    let rows = side_by_side_rows(&app.document);
    for (offset, row) in rows
        .iter()
        .skip(app.content_scroll)
        .take(area.height as usize)
        .enumerate()
    {
        let y = area.y + offset as u16;
        draw_diff_side(
            frame,
            Rect::new(left_area.x, y, left_area.width.saturating_sub(1), 1),
            row.0,
            true,
            app.horizontal_scroll,
            theme,
        );
        draw_diff_side(
            frame,
            Rect::new(right_area.x, y, right_area.width, 1),
            row.1,
            false,
            app.horizontal_scroll,
            theme,
        );
    }
}

type DiffPair<'a> = (Option<&'a DiffLine>, Option<&'a DiffLine>);

fn side_by_side_rows(document: &DiffDocument) -> Vec<DiffPair<'_>> {
    let mut rows = Vec::new();
    let mut index = 0;
    while index < document.lines.len() {
        let line = &document.lines[index];
        if line.kind != DiffLineKind::Removed {
            if line.kind == DiffLineKind::Added {
                rows.push((None, Some(line)));
            } else {
                rows.push((Some(line), Some(line)));
            }
            index += 1;
            continue;
        }

        let removed_start = index;
        while index < document.lines.len() && document.lines[index].kind == DiffLineKind::Removed {
            index += 1;
        }
        let added_start = index;
        while index < document.lines.len() && document.lines[index].kind == DiffLineKind::Added {
            index += 1;
        }
        let removed = &document.lines[removed_start..added_start];
        let added = &document.lines[added_start..index];
        for pair_index in 0..removed.len().max(added.len()) {
            rows.push((removed.get(pair_index), added.get(pair_index)));
        }
    }
    rows
}

fn draw_diff_side(
    frame: &mut Frame<'_>,
    area: Rect,
    line: Option<&DiffLine>,
    old_side: bool,
    horizontal_scroll: usize,
    theme: &Theme,
) {
    let Some(line) = line else {
        frame.render_widget(
            Paragraph::new("░").style(Style::default().fg(theme.border).bg(theme.panel_alt)),
            area,
        );
        return;
    };
    let number = if old_side {
        line.old_line
    } else {
        line.new_line
    };
    let number = number.map_or(String::new(), |number| number.to_string());
    let (marker, marker_style) = marker_for(line.kind, theme);
    let mut spans = vec![
        Span::styled(format!("{number:>4} "), Style::default().fg(theme.muted)),
        Span::styled(marker, marker_style),
    ];
    spans.extend(highlight_spans(
        &line.spans,
        horizontal_scroll,
        area.width.saturating_sub(7) as usize,
        line.kind,
        theme,
    ));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(line_background(line.kind, theme)),
        area,
    );
}

fn highlight_spans<'a>(
    spans: &'a [HighlightSpan],
    horizontal_scroll: usize,
    width: usize,
    kind: DiffLineKind,
    theme: &Theme,
) -> Vec<Span<'a>> {
    let mut skip = horizontal_scroll;
    let mut remaining = width;
    let mut output = Vec::new();
    for span in spans {
        if remaining == 0 {
            break;
        }
        let text_width = span.text.width();
        if skip >= text_width {
            skip -= text_width;
            continue;
        }
        let sliced = slice_width(&span.text, skip, remaining);
        skip = 0;
        remaining = remaining.saturating_sub(sliced.width());
        let foreground = span
            .foreground
            .map(|(r, g, b)| Color::Rgb(r, g, b))
            .unwrap_or_else(|| line_foreground(kind, theme));
        let mut style = Style::default().fg(foreground);
        if span.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if span.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }
        output.push(Span::styled(sliced, style));
    }
    output
}

fn marker_for(kind: DiffLineKind, theme: &Theme) -> (&'static str, Style) {
    match kind {
        DiffLineKind::Added => (
            "+ ",
            Style::default()
                .fg(theme.added)
                .add_modifier(Modifier::BOLD),
        ),
        DiffLineKind::Removed => (
            "- ",
            Style::default()
                .fg(theme.removed)
                .add_modifier(Modifier::BOLD),
        ),
        DiffLineKind::HunkHeader => (
            "◆ ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        DiffLineKind::FileHeader => ("▸ ", Style::default().fg(theme.modified)),
        DiffLineKind::Context => ("  ", Style::default().fg(theme.muted)),
        DiffLineKind::Meta => ("  ", Style::default().fg(theme.muted)),
    }
}

fn line_background(kind: DiffLineKind, theme: &Theme) -> Style {
    match kind {
        DiffLineKind::Added => Style::default().bg(theme.added_background),
        DiffLineKind::Removed => Style::default().bg(theme.removed_background),
        DiffLineKind::HunkHeader => Style::default().bg(theme.panel_alt).fg(theme.accent),
        _ => Style::default().bg(theme.panel),
    }
}

fn line_foreground(kind: DiffLineKind, theme: &Theme) -> Color {
    match kind {
        DiffLineKind::Added => theme.added,
        DiffLineKind::Removed => theme.removed,
        DiffLineKind::HunkHeader => theme.accent,
        DiffLineKind::FileHeader => theme.modified,
        _ => theme.text,
    }
}

fn draw_scrollbar(frame: &mut Frame<'_>, area: Rect, offset: usize, length: usize, theme: &Theme) {
    if length <= area.height as usize || area.width == 0 {
        return;
    }
    let height = area.height as usize;
    let thumb_height = (height * height / length).max(1).min(height);
    let max_offset = length.saturating_sub(height).max(1);
    let thumb_start = offset.min(max_offset) * (height - thumb_height) / max_offset;
    for row in 0..height {
        let color = if (thumb_start..thumb_start + thumb_height).contains(&row) {
            theme.accent_soft
        } else {
            theme.border
        };
        frame.render_widget(
            Paragraph::new("▐").style(Style::default().fg(color)),
            Rect::new(area.right().saturating_sub(1), area.y + row as u16, 1, 1),
        );
    }
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let left = if let Some(busy) = app.busy.as_deref() {
        Line::from(vec![
            Span::styled(
                " ⟳ ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(busy, Style::default().fg(theme.text)),
        ])
    } else if app.refreshing {
        Line::from(vec![
            Span::styled(" ⟳ ", Style::default().fg(theme.accent)),
            Span::styled("Refreshing repository…", Style::default().fg(theme.muted)),
        ])
    } else {
        Line::from(vec![
            Span::styled("  ", Style::default().fg(theme.accent)),
            Span::styled(
                if app.status.branch.head.is_empty() {
                    "—"
                } else {
                    &app.status.branch.head
                },
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "   {} changes   {} staged",
                    app.status.changes.len(),
                    app.status.staged_count()
                ),
                Style::default().fg(theme.muted),
            ),
        ])
    };
    let right = match app.view {
        View::Changes => " s stage  u unstage  c commit  b branch  ? help ",
        View::History => " C cherry-pick  R revert  n branch  ? help ",
    };
    let regions = Layout::horizontal([
        Constraint::Min(20),
        Constraint::Length(right.width() as u16),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(left)
            .style(Style::default().bg(theme.panel_alt))
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(theme.border)),
            ),
        regions[0],
    );
    frame.render_widget(
        Paragraph::new(right)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.muted).bg(theme.panel_alt))
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(theme.border)),
            ),
        regions[1],
    );
}

fn draw_modal(frame: &mut Frame<'_>, modal: &Modal, app: &App, theme: &Theme) {
    match modal {
        Modal::Help { scroll } => draw_help(frame, *scroll, theme),
        Modal::Commit { input, amend } => draw_commit(frame, input, *amend, theme),
        Modal::Prompt { title, input, .. } => draw_prompt(frame, title, input, theme),
        Modal::Confirm { title, message, .. } => draw_confirm(frame, title, message, theme),
        Modal::Branches {
            items,
            selected,
            query,
            loading,
            ..
        } => draw_branches(frame, items, *selected, query, *loading, theme),
        Modal::CommandPalette { query, selected } => {
            draw_palette(frame, app, query, *selected, theme);
        }
        Modal::Conflict { change } => draw_conflict(frame, change, theme),
    }
}

fn draw_help(frame: &mut Frame<'_>, scroll: usize, theme: &Theme) {
    let area = centered_rect(
        68,
        31.min(frame.area().height.saturating_sub(4)),
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Keyboard Shortcuts ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = HELP_LINES
        .iter()
        .skip(scroll)
        .take(inner.height.saturating_sub(1) as usize)
        .map(|(key, description)| {
            if description.is_empty() && !key.is_empty() {
                Line::from(Span::styled(
                    *key,
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(vec![
                    Span::styled(format!("{key:<20}"), Style::default().fg(theme.modified)),
                    Span::styled(*description, Style::default().fg(theme.text)),
                ])
            }
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
    draw_modal_hint(frame, area, "Esc close", theme);
}

fn draw_commit(frame: &mut Frame<'_>, input: &crate::app::TextBuffer, amend: bool, theme: &Theme) {
    let width = frame.area().width.saturating_sub(12).min(76);
    let area = centered_rect(width, 12, frame.area());
    frame.render_widget(Clear, area);
    let block = modal_block(
        if amend {
            " Amend Commit "
        } else {
            " Commit Staged Changes "
        },
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let input_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(3),
    );
    frame.render_widget(
        Paragraph::new(input.value.as_str())
            .style(Style::default().fg(theme.text).bg(theme.panel_alt))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_focus)),
            ),
        input_area,
    );
    set_text_cursor(frame, input_area.inner(Margin::new(1, 1)), input, true);
    frame.render_widget(
        Paragraph::new("Ctrl+Enter commit   Enter newline   Esc cancel")
            .style(Style::default().fg(theme.muted)),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn draw_prompt(frame: &mut Frame<'_>, title: &str, input: &crate::app::TextBuffer, theme: &Theme) {
    let area = centered_rect(
        frame.area().width.saturating_sub(14).min(68),
        7,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(&format!(" {title} "), theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let input_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    frame.render_widget(
        Paragraph::new(input.value.as_str())
            .style(Style::default().fg(theme.text).bg(theme.panel_alt)),
        input_area,
    );
    set_text_cursor(frame, input_area, input, false);
    draw_modal_hint(frame, area, "Enter accept   Esc cancel", theme);
}

fn draw_confirm(frame: &mut Frame<'_>, title: &str, message: &str, theme: &Theme) {
    let area = centered_rect(
        frame.area().width.saturating_sub(14).min(72),
        9,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(&format!(" {title} "), theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(theme.text))
            .wrap(Wrap { trim: true }),
        Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(3),
        ),
    );
    draw_modal_hint(frame, area, "y / Enter confirm   n / Esc cancel", theme);
}

fn draw_conflict(frame: &mut Frame<'_>, change: &Change, theme: &Theme) {
    let area = centered_rect(
        frame.area().width.saturating_sub(14).min(72),
        10,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Resolve Merge Conflict ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                change.display_path(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "o",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" accept ours     "),
                Span::styled(
                    "t",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" accept theirs     "),
                Span::styled(
                    "s",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" mark resolved"),
            ]),
        ]),
        Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(2),
        ),
    );
    draw_modal_hint(frame, area, "Esc cancel", theme);
}

fn draw_branches(
    frame: &mut Frame<'_>,
    items: &[Branch],
    selected: usize,
    query: &crate::app::TextBuffer,
    loading: bool,
    theme: &Theme,
) {
    let height = frame.area().height.saturating_sub(8).min(25);
    let area = centered_rect(
        frame.area().width.saturating_sub(12).min(76),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Branches ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );

    let visible = App::filtered_branches(items, &query.value);
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    if loading {
        frame.render_widget(
            Paragraph::new("Loading branches…").style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
        let lines = visible
            .iter()
            .skip(offset)
            .take(list_area.height as usize)
            .enumerate()
            .filter_map(|(visible_offset, index)| {
                let branch = items.get(*index)?;
                let cursor = offset + visible_offset;
                let style = if cursor == selected {
                    Style::default()
                        .bg(theme.selected)
                        .fg(theme.text)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text)
                };
                Some(Line::from(vec![
                    Span::styled(
                        if branch.current { " ● " } else { "   " },
                        Style::default()
                            .fg(if branch.current {
                                theme.success
                            } else {
                                theme.muted
                            })
                            .bg(style.bg.unwrap_or(theme.panel)),
                    ),
                    Span::styled(
                        truncate_middle(&branch.name, list_area.width.saturating_sub(30) as usize),
                        style,
                    ),
                    Span::styled(
                        format!("  {}  {}", branch.short_id, branch.relative_date),
                        Style::default()
                            .fg(theme.muted)
                            .bg(style.bg.unwrap_or(theme.panel)),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(lines), list_area);
    }
    draw_modal_hint(
        frame,
        area,
        "Enter switch   Ctrl+n new   Delete delete   Esc close",
        theme,
    );
}

fn draw_palette(
    frame: &mut Frame<'_>,
    app: &App,
    query: &crate::app::TextBuffer,
    selected: usize,
    theme: &Theme,
) {
    let commands = app.palette_commands(&query.value);
    let height = (commands.len() as u16 + 6)
        .min(frame.area().height.saturating_sub(6))
        .max(8);
    let area = Rect::new(
        frame.area().x
            + (frame
                .area()
                .width
                .saturating_sub(76.min(frame.area().width.saturating_sub(8))))
                / 2,
        frame.area().y + 3,
        76.min(frame.area().width.saturating_sub(8)),
        height,
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Command Palette ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let query_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" > ", Style::default().fg(theme.accent)),
            Span::styled(query.value.as_str(), Style::default().fg(theme.text)),
        ]))
        .style(Style::default().bg(theme.panel_alt)),
        query_area,
    );
    set_text_cursor(
        frame,
        Rect::new(
            query_area.x + 3,
            query_area.y,
            query_area.width.saturating_sub(3),
            1,
        ),
        query,
        false,
    );
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(2),
    );
    let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
    let lines = commands
        .iter()
        .skip(offset)
        .take(list_area.height as usize)
        .enumerate()
        .map(|(index, command)| palette_line(*command, offset + index == selected, theme))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), list_area);
}

fn palette_line(command: PaletteCommand, selected: bool, theme: &Theme) -> Line<'static> {
    let background = if selected {
        theme.selected
    } else {
        theme.panel
    };
    Line::from(vec![
        Span::styled(
            if selected { " › " } else { "   " },
            Style::default().fg(theme.accent).bg(background),
        ),
        Span::styled(
            command.label(),
            Style::default()
                .fg(theme.text)
                .bg(background)
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
    ])
}

fn draw_toast(frame: &mut Frame<'_>, message: &str, level: ToastLevel, theme: &Theme) {
    let width = (message.width() as u16 + 6)
        .min(frame.area().width.saturating_sub(4))
        .max(24);
    let height = ((message.width() as u16 / width.max(1)) + 3).min(7);
    let area = Rect::new(
        frame.area().right().saturating_sub(width + 2),
        frame.area().bottom().saturating_sub(height + 3),
        width,
        height,
    );
    let color = match level {
        ToastLevel::Info => theme.accent,
        ToastLevel::Success => theme.success,
        ToastLevel::Error => theme.error,
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(theme.text).bg(theme.panel_alt))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(color)),
            ),
        area,
    );
}

fn draw_modal_hint(frame: &mut Frame<'_>, area: Rect, hint: &str, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(hint)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.muted).bg(theme.panel)),
        Rect::new(
            area.x + 2,
            area.bottom().saturating_sub(2),
            area.width.saturating_sub(4),
            1,
        ),
    );
}

fn set_text_cursor(
    frame: &mut Frame<'_>,
    area: Rect,
    input: &crate::app::TextBuffer,
    multiline: bool,
) {
    let before = &input.value[..input.cursor.min(input.value.len())];
    let (row, column) = if multiline {
        let row = before
            .chars()
            .filter(|character| *character == '\n')
            .count();
        let column = before.rsplit('\n').next().unwrap_or_default().width();
        (row, column)
    } else {
        (0, before.replace('\n', " ").width())
    };
    let x = area
        .x
        .saturating_add(column.min(area.width.saturating_sub(1) as usize) as u16);
    let y = area
        .y
        .saturating_add(row.min(area.height.saturating_sub(1) as usize) as u16);
    frame.set_cursor_position((x, y));
}

fn panel_block(title: String, focused: bool, theme: &Theme) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(if focused {
            theme.border_focus
        } else {
            theme.border
        }))
        .style(Style::default().bg(theme.panel).fg(theme.text))
}

fn modal_block(title: &str, theme: &Theme) -> Block<'static> {
    Block::default()
        .title(title.to_owned())
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_focus))
        .style(Style::default().bg(theme.panel).fg(theme.text))
}

fn status_color(status: ChangeStatus, theme: &Theme) -> Color {
    match status {
        ChangeStatus::Added | ChangeStatus::Untracked => theme.added,
        ChangeStatus::Deleted => theme.removed,
        ChangeStatus::Conflicted => theme.conflict,
        ChangeStatus::Modified
        | ChangeStatus::Renamed
        | ChangeStatus::Copied
        | ChangeStatus::TypeChanged => theme.modified,
    }
}

fn graph_color(index: usize, theme: &Theme) -> Color {
    match index % 4 {
        0 => theme.accent,
        1 => theme.modified,
        2 => theme.added,
        _ => theme.conflict,
    }
}

fn history_glyph(commit: &crate::git::history::Commit, index: usize) -> &'static str {
    if commit.parent_ids.len() > 1 {
        "●╮ "
    } else if index > 0 {
        "●│ "
    } else {
        "●  "
    }
}

fn clean_decoration(decoration: &str) -> &str {
    decoration.strip_prefix("HEAD -> ").unwrap_or(decoration)
}

fn ensure_offset(offset: &mut usize, cursor: usize, height: usize, length: usize) {
    if height == 0 || length == 0 {
        *offset = 0;
        return;
    }
    if cursor < *offset {
        *offset = cursor;
    } else if cursor >= *offset + height {
        *offset = cursor + 1 - height;
    }
    *offset = (*offset).min(length.saturating_sub(height));
}

fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let width = width.min(outer.width);
    let height = height.min(outer.height);
    Rect::new(
        outer.x + (outer.width - width) / 2,
        outer.y + (outer.height - height) / 2,
        width,
        height,
    )
}

fn truncate_middle(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if value.width() <= width {
        return value.to_owned();
    }
    if width <= 1 {
        return "…".to_owned();
    }
    let left_width = (width - 1) * 2 / 3;
    let right_width = width - 1 - left_width;
    format!(
        "{}…{}",
        slice_width(value, 0, left_width),
        suffix_width(value, right_width)
    )
}

fn slice_width(value: &str, skip: usize, width: usize) -> String {
    let mut skipped = 0;
    let mut used = 0;
    let mut output = String::new();
    for character in value.chars() {
        let character_width = character.width().unwrap_or_default();
        if skipped + character_width <= skip {
            skipped += character_width;
            continue;
        }
        if used + character_width > width {
            break;
        }
        output.push(character);
        used += character_width;
    }
    output
}

fn suffix_width(value: &str, width: usize) -> String {
    let mut characters = Vec::new();
    let mut used = 0;
    for character in value.chars().rev() {
        let character_width = character.width().unwrap_or_default();
        if used + character_width > width {
            break;
        }
        characters.push(character);
        used += character_width;
    }
    characters.into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_truncation_respects_display_width() {
        let result = truncate_middle("src/a-very-long-file-name.rs", 14);
        assert!(result.width() <= 14);
        assert!(result.contains('…'));
        assert!(result.ends_with("me.rs"));
    }

    #[test]
    fn side_by_side_pairs_replacements() {
        let document = DiffDocument {
            title: String::new(),
            truncated: false,
            lines: vec![
                test_line(DiffLineKind::Removed, "old one"),
                test_line(DiffLineKind::Removed, "old two"),
                test_line(DiffLineKind::Added, "new one"),
                test_line(DiffLineKind::Context, "same"),
            ],
        };
        let rows = side_by_side_rows(&document);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].0.unwrap().text(), "old one");
        assert_eq!(rows[0].1.unwrap().text(), "new one");
        assert!(rows[1].1.is_none());
        assert_eq!(rows[2].0.unwrap().text(), "same");
    }

    fn test_line(kind: DiffLineKind, text: &str) -> DiffLine {
        DiffLine {
            kind,
            old_line: None,
            new_line: None,
            spans: vec![HighlightSpan {
                text: text.to_owned(),
                foreground: None,
                bold: false,
                italic: false,
            }],
        }
    }
}
