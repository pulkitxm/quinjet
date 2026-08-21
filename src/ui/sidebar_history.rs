#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
pub(super) fn draw_history_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> Vec<SidebarHitArea> {
    let title = if app.filter.is_empty() {
        format!(
            " History · {}  {}{}  [b branch] ",
            app.history_branch_label(),
            app.history.len(),
            if app.history_complete { "" } else { "+" }
        )
    } else {
        format!(
            " History · {}  /{} ",
            app.history_branch_label(),
            app.filter
        )
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
    app.sidebar_viewport(app.history_cursor, height, visible.len());
    let mut hits = Vec::new();
    let end = (app.sidebar_offset + height).min(visible.len());
    for (row_offset, index) in visible
        .iter()
        .take(end)
        .skip(app.sidebar_offset)
        .enumerate()
    {
        let cursor = app.sidebar_offset + row_offset;
        let Some(commit) = app.history.get(*index) else {
            continue;
        };
        let selected = cursor == app.history_cursor;
        let y = inner.y + cells(row_offset);
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
        let sha_area = Rect::new(
            inner
                .right()
                .saturating_sub(1)
                .saturating_sub(cells(commit.short_id.width())),
            y,
            cells(commit.short_id.width()),
            1,
        );
        frame.render_widget(
            Paragraph::new(Line::from(link_span(
                commit.short_id.clone(),
                app.commit_open_target(&commit.id),
                sha_area,
                theme,
                link_hits,
            )))
            .alignment(Alignment::Right)
            .style(Style::default().bg(if selected {
                theme.selected
            } else {
                theme.panel
            })),
            Rect::new(inner.right().saturating_sub(10), y, 9, 1),
        );
        hits.push(SidebarHitArea {
            area: Rect::new(inner.x, y, inner.width, 1),
            target: SidebarHit::Commit(*index),
        });
    }

    draw_scrollbar(frame, inner, app.sidebar_offset, visible.len(), theme);

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
