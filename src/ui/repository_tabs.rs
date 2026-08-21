#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::integer_division,
    reason = "tab widths use whole terminal cells"
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
    let open_width = 5_u16.min(area.width);
    let open = Rect::new(
        area.right().saturating_sub(open_width),
        area.y,
        open_width,
        area.height,
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
    let tab_region_width = area.width.saturating_sub(open_width);
    if app.repository_tabs.is_empty() || tab_region_width == 0 {
        return (Vec::new(), open, Rect::default(), Rect::default());
    }
    let minimum_width = 10_u16.min(tab_region_width.max(1));
    let base_capacity = usize::from((tab_region_width / minimum_width).max(1));
    let overflow = app.repository_tabs.len() > base_capacity;
    let control_width = if overflow { 3 } else { 0 };
    let (previous, next) = if overflow {
        (
            Rect::new(area.x, area.y, control_width, area.height),
            Rect::new(
                open.x.saturating_sub(control_width),
                area.y,
                control_width,
                area.height,
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
        let x = area
            .x
            .saturating_add(control_width)
            .saturating_add(cells(offset.saturating_mul(usize::from(tab_width))));
        let tab_area = Rect::new(x, area.y, tab_width, area.height);
        let value = if tab.title.is_empty() {
            tab.root.display().to_string()
        } else {
            tab.title.clone()
        };
        let label = format!(
            " {} ",
            truncate_end(&value, usize::from(tab_width.saturating_sub(2)))
        );
        let style = if tab.active {
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
                .style(style),
            tab_area,
        );
        hits.push(RepositoryTabHit {
            area: tab_area,
            id: tab.id,
        });
    }
    (hits, open, previous, next)
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
