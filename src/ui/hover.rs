#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use crate::app::DiffViewport;

pub(super) fn diff_viewport(
    app: &App,
    area: Rect,
    scroll: usize,
    side_by_side: bool,
) -> DiffViewport {
    let first = pull_request_viewport::diff_row_line_index(app, side_by_side, scroll);
    let sticky = first.is_some_and(|index| {
        app.document
            .lines
            .get(index)
            .is_some_and(|line| line.kind != DiffLineKind::FileHeader)
            && app.preview_header_at(index).is_some()
    });
    DiffViewport {
        area: Rect::new(
            area.x,
            area.y + u16::from(sticky),
            area.width,
            area.height.saturating_sub(u16::from(sticky)),
        ),
        scroll,
        side_by_side,
    }
}

pub(super) fn draw(frame: &mut Frame<'_>, app: &App, theme: &Theme) {
    let Some(point) = app.mouse_hover.map(Into::into) else {
        return;
    };
    if app.modal.is_some()
        || app.repository_tab_menu.is_some()
        || app.scm_menu_open
        || app.pr_menu_open
        || app.resize_target.is_some()
        || app.scrollbar_drag.is_some()
    {
        return;
    }
    if let Some(hit) = app
        .geometry
        .sidebar_hits
        .iter()
        .find(|hit| hit.area.contains(point))
    {
        tint(frame, hit.area, theme);
        return;
    }
    if let Some(hit) = app
        .geometry
        .stack_inspector_hits
        .iter()
        .find(|hit| hit.area.contains(point))
    {
        tint(frame, hit.area, theme);
        return;
    }
    if let Some(viewport) = app.geometry.diff_viewport
        && viewport.area.contains(point)
    {
        draw_diff(frame, app, viewport, point.y, theme);
        return;
    }
    let content = app.geometry.content.inner(Margin {
        horizontal: 0,
        vertical: 1,
    });
    if content.contains(point) {
        let area = Rect::new(content.x, point.y, content.width.saturating_sub(1), 1);
        let has_text = (area.x..area.right()).any(|x| {
            frame
                .buffer_mut()
                .cell((x, point.y))
                .is_some_and(|cell| !cell.symbol().trim().is_empty())
        });
        if has_text {
            tint(frame, area, theme);
        }
    }
}

fn diff_indices(app: &App, viewport: DiffViewport, row: usize) -> [Option<usize>; 2] {
    if viewport.side_by_side {
        match app.side_by_side_diff_rows.get(row) {
            Some(SideBySideRow::Split(old, new)) => [*old, *new],
            Some(SideBySideRow::FileHeader(index) | SideBySideRow::Full { index, .. }) => {
                [Some(*index), None]
            }
            _ => [None, None],
        }
    } else {
        [app.unified_diff_rows.get(row).copied(), None]
    }
}

fn draw_diff(frame: &mut Frame<'_>, app: &App, viewport: DiffViewport, y: u16, theme: &Theme) {
    let hovered = diff_indices(
        app,
        viewport,
        viewport.scroll + usize::from(y - viewport.area.y),
    );
    let hovered: Vec<usize> = hovered
        .into_iter()
        .flatten()
        .filter(|index| {
            app.document.lines.get(*index).is_some_and(|line| {
                !matches!(line.kind, DiffLineKind::Image | DiffLineKind::FileFooter)
            })
        })
        .collect();
    if hovered.is_empty() {
        return;
    }
    for offset in 0..viewport.area.height {
        if diff_indices(app, viewport, viewport.scroll + usize::from(offset))
            .into_iter()
            .flatten()
            .any(|index| hovered.contains(&index))
        {
            tint(
                frame,
                Rect::new(
                    viewport.area.x,
                    viewport.area.y + offset,
                    viewport.area.width,
                    1,
                ),
                theme,
            );
        }
    }
}

fn tint(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    for row in area.y..area.bottom() {
        for column in area.x..area.right() {
            if let Some(cell) = frame.buffer_mut().cell_mut((column, row))
                && cell.bg != theme.selected
            {
                cell.bg = hover_color(cell.bg, theme.accent_soft);
            }
        }
    }
}

pub(super) const fn hover_color(background: Color, accent: Color) -> Color {
    if let (Color::Rgb(r, g, b), Color::Rgb(ar, ag, ab)) = (background, accent) {
        Color::Rgb(blend(r, ar), blend(g, ag), blend(b, ab))
    } else {
        accent
    }
}

const fn blend(background: u8, accent: u8) -> u8 {
    background
        .saturating_sub(background.div_euclid(2))
        .saturating_add(accent.div_euclid(2))
}
