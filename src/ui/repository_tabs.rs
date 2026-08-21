#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::integer_division,
    reason = "tab widths use whole terminal cells"
)]
#[expect(
    clippy::too_many_lines,
    reason = "tab layout and hit targets share one coordinate pass"
)]
pub(super) fn draw_repository_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    theme: &Theme,
) -> (Vec<RepositoryTabHit>, Rect, Rect, Rect) {
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.panel)),
        area,
    );
    let tab_row = Rect::new(area.x, area.y, area.width, 1_u16.min(area.height));
    let separator = Rect::new(
        area.x,
        tab_row.bottom(),
        area.width,
        area.bottom().saturating_sub(tab_row.bottom()).min(1),
    );
    frame.render_widget(
        Paragraph::new("─".repeat(usize::from(separator.width)))
            .style(Style::default().fg(theme.border).bg(theme.panel)),
        separator,
    );
    let open_width = 5_u16.min(tab_row.width);
    let open = Rect::new(
        tab_row.right().saturating_sub(open_width),
        tab_row.y,
        open_width,
        tab_row.height,
    );
    frame.render_widget(
        Paragraph::new(" + ").alignment(Alignment::Center).style(
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        ),
        open,
    );
    let tab_region_width = tab_row.width.saturating_sub(open_width);
    if app.repository_tabs.is_empty() || tab_region_width == 0 {
        return (Vec::new(), open, Rect::default(), Rect::default());
    }
    let minimum_width = 12_u16.min(tab_region_width.max(1));
    let base_capacity = usize::from((tab_region_width / minimum_width).max(1));
    let overflow = app.repository_tabs.len() > base_capacity;
    let control_width = if overflow { 3 } else { 0 };
    let (previous, next) = if overflow {
        (
            Rect::new(tab_row.x, tab_row.y, control_width, tab_row.height),
            Rect::new(
                open.x.saturating_sub(control_width),
                tab_row.y,
                control_width,
                tab_row.height,
            ),
        )
    } else {
        (Rect::default(), Rect::default())
    };
    if overflow {
        let control_style = Style::default()
            .fg(theme.accent)
            .bg(theme.panel)
            .add_modifier(Modifier::BOLD);
        frame.render_widget(
            Paragraph::new("‹")
                .alignment(Alignment::Center)
                .style(control_style),
            previous,
        );
        frame.render_widget(
            Paragraph::new("›")
                .alignment(Alignment::Center)
                .style(control_style),
            next,
        );
    }
    let available = tab_region_width.saturating_sub(control_width.saturating_mul(2));
    let capacity = usize::from((available / minimum_width).max(1));
    let active = app
        .repository_tabs
        .iter()
        .position(|tab| tab.active)
        .unwrap_or_default();
    let start = active.saturating_add(1).saturating_sub(capacity);
    let visible_count = app
        .repository_tabs
        .len()
        .saturating_sub(start)
        .min(capacity);
    let tab_width = (available / u16::try_from(visible_count).unwrap_or(1)).min(24);
    let mut hits = Vec::with_capacity(visible_count);
    for (offset, tab) in app
        .repository_tabs
        .iter()
        .skip(start)
        .take(visible_count)
        .enumerate()
    {
        let x = tab_row
            .x
            .saturating_add(control_width)
            .saturating_add(cells(offset.saturating_mul(usize::from(tab_width))));
        let tab_area = Rect::new(x, tab_row.y, tab_width, tab_row.height);
        let value = if tab.title.is_empty() {
            tab.root.display().to_string()
        } else {
            tab.title.clone()
        };
        let label_width = tab_width.saturating_sub(4);
        let label = format!(
            " {}",
            truncate_end(&value, usize::from(label_width.saturating_sub(1)))
        );
        let style = if tab.active {
            Style::default()
                .fg(theme.text)
                .bg(theme.accent_soft)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted).bg(theme.panel_alt)
        };
        let label_area = Rect::new(tab_area.x, tab_area.y, label_width, tab_area.height);
        let close = Rect::new(
            tab_area.right().saturating_sub(4),
            tab_area.y,
            3_u16.min(tab_area.width),
            tab_area.height,
        );
        frame.render_widget(
            Paragraph::new(label)
                .alignment(Alignment::Left)
                .style(style),
            label_area,
        );
        frame.render_widget(
            Paragraph::new("×")
                .alignment(Alignment::Center)
                .style(style),
            close,
        );
        frame.render_widget(
            Paragraph::new("│").style(Style::default().fg(theme.border).bg(if tab.active {
                theme.accent_soft
            } else {
                theme.panel_alt
            })),
            Rect::new(
                tab_area.right().saturating_sub(1),
                tab_area.y,
                1,
                tab_area.height,
            ),
        );
        hits.push(RepositoryTabHit {
            area: tab_area,
            close,
            id: tab.id,
        });
    }
    draw_repository_tab_drop_marker(frame, tab_row, app, &hits, theme);
    (hits, open, previous, next)
}

fn draw_repository_tab_drop_marker(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    hits: &[RepositoryTabHit],
    theme: &Theme,
) {
    let Some(drag) = app.repository_tab_drag else {
        return;
    };
    let Some(target) = drag.target else {
        return;
    };
    let Some(source_index) = app.repository_tabs.iter().position(|tab| tab.id == drag.id) else {
        return;
    };
    let Some(target_index) = app.repository_tabs.iter().position(|tab| tab.id == target) else {
        return;
    };
    let Some(target_hit) = hits.iter().find(|hit| hit.id == target) else {
        return;
    };
    let column = if source_index < target_index {
        target_hit.area.right().min(area.right().saturating_sub(1))
    } else {
        target_hit.area.x
    };
    frame.render_widget(
        Paragraph::new("▏").style(Style::default().fg(Color::White).bg(theme.panel_alt)),
        Rect::new(column, area.y, 1, area.height),
    );
}

pub(super) fn draw_repository_tab_menu(frame: &mut Frame<'_>, app: &mut App, theme: &Theme) {
    app.geometry.repository_tab_menu_hits.clear();
    let Some(menu) = app.repository_tab_menu else {
        return;
    };
    let outer = frame.area();
    let width = 22_u16.min(outer.width);
    let height = 6_u16.min(outer.height);
    let x = menu
        .column
        .min(outer.right().saturating_sub(width).max(outer.x));
    let y = menu
        .row
        .saturating_add(1)
        .min(outer.bottom().saturating_sub(height).max(outer.y));
    let area = Rect::new(x, y, width, height);
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focus))
        .style(Style::default().bg(theme.panel));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    for (index, action) in RepositoryTabAction::ALL.iter().copied().enumerate() {
        let row = Rect::new(
            inner.x,
            inner.y.saturating_add(cells(index)),
            inner.width,
            1,
        );
        let style = if index == menu.selected {
            Style::default().fg(theme.text).bg(theme.selected)
        } else {
            Style::default().fg(theme.text).bg(theme.panel)
        };
        frame.render_widget(
            Paragraph::new(format!(" {}", action.label())).style(style),
            row,
        );
        app.geometry.repository_tab_menu_hits.push((row, action));
    }
}
