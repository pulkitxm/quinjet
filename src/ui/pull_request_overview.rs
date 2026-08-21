#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " A pre-wrapped content row, optionally anchored to a check step so a click or"]
#[doc = " the step cursor can find it after scrolling."]
pub(super) type ContentRow = PullRequestContentRow;

pub(super) type ContentLink = PullRequestContentLink;

#[derive(Clone)]
pub(super) struct Link {
    target: OpenTarget,
}

impl Link {
    pub(super) const fn new(target: OpenTarget) -> Self {
        Self { target }
    }

    pub(super) fn register(&self, area: Rect, hits: &mut Vec<LinkHit>) {
        if area.width > 0 && area.height > 0 {
            hits.push(LinkHit {
                area,
                target: self.target.clone(),
            });
        }
    }

    pub(super) fn style(theme: &Theme) -> Style {
        link_style(theme)
    }

    pub(super) fn span(
        self,
        text: String,
        area: Rect,
        theme: &Theme,
        hits: &mut Vec<LinkHit>,
    ) -> Span<'static> {
        self.register(area, hits);
        Span::styled(text, Self::style(theme))
    }
}

impl PullRequestContentRow {
    pub(super) const fn plain(line: Line<'static>) -> Self {
        Self {
            line,
            step: None,
            wide: false,
        }
    }

    pub(super) const fn wide(line: Line<'static>) -> Self {
        Self {
            line,
            step: None,
            wide: true,
        }
    }

    pub(super) fn blank() -> Self {
        Self::plain(Line::default())
    }

    pub(super) fn text(value: impl Into<String>, style: Style) -> Self {
        Self::plain(Line::from(Span::styled(value.into(), style)))
    }
}

pub(super) fn draw_pull_request_overview(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> Vec<ContentStepHit> {
    let showing_check = app.pull_request_check_cursor.is_some();
    let focused = app.focus == Focus::Content && app.modal.is_none();
    let inner = panel_block(String::new(), focused, theme).inner(area);
    if inner.width == 0 || inner.height == 0 {
        frame.render_widget(
            panel_block(overview_title(app, showing_check), focused, theme),
            area,
        );
        return Vec::new();
    }

    let width = inner.width as usize;
    let rows_key = (showing_check, width, app.pull_request_content_generation);
    if app.pull_request_content_rows_key != Some(rows_key) {
        app.pull_request_content_rows = if showing_check {
            check_run_rows(app, width, theme)
        } else {
            conversation_rows(app, width, theme)
        };
        app.pull_request_content_width = app
            .pull_request_content_rows
            .iter()
            .filter(|row| row.wide)
            .map(|row| row.line.width())
            .max()
            .unwrap_or_default();
        app.pull_request_content_links =
            pull_request_content_links(app, showing_check, &app.pull_request_content_rows);
        app.pull_request_content_rows_key = Some(rows_key);
    }
    let rows = &app.pull_request_content_rows;
    let row_links = &app.pull_request_content_links;
    let content_width = app.pull_request_content_width;
    let overflow = content_width.saturating_sub(width);
    app.horizontal_scroll = app.horizontal_scroll.min(overflow);

    let mut title = overview_title(app, showing_check);
    if overflow > 0 {
        title.push_str("·  ←/→ ");
        title.push_str(&app.horizontal_scroll.to_string());
        title.push('/');
        title.push_str(&overflow.to_string());
        title.push(' ');
    }
    frame.render_widget(panel_block(title, focused, theme), area);

    if showing_check && app.pull_request_step_reveal {
        app.pull_request_step_reveal = false;
        if let Some(cursor_row) = rows
            .iter()
            .position(|row| row.step == Some(app.pull_request_step_cursor))
        {
            ensure_offset(
                &mut app.content_scroll,
                cursor_row,
                inner.height as usize,
                rows.len(),
            );
        }
    }
    let max_scroll = rows.len().saturating_sub(inner.height as usize);
    app.content_scroll = app.content_scroll.min(max_scroll);
    app.content_at_bottom = app.content_scroll == max_scroll;

    let mut hits = Vec::new();
    for (offset, row) in rows
        .iter()
        .skip(app.content_scroll)
        .take(inner.height as usize)
        .enumerate()
    {
        let source_row = app.content_scroll.saturating_add(offset);
        let row_area = Rect::new(inner.x, inner.y + cells(offset), inner.width, 1);
        let selected = showing_check && row.step == Some(app.pull_request_step_cursor);
        frame.render_widget(
            Paragraph::new(if row.wide {
                shift_line(&row.line, app.horizontal_scroll, width)
            } else {
                row.line.clone()
            })
            .style(Style::default().bg(if selected {
                theme.selected
            } else {
                theme.panel
            })),
            row_area,
        );
        if let Some(step) = row.step {
            hits.push(ContentStepHit {
                area: row_area,
                step,
            });
        }
        for link in row_links.iter().filter(|link| link.row == source_row) {
            let area = horizontally_scrolled_link_area(
                row_area,
                app.horizontal_scroll,
                link.start,
                link.width,
            );
            Link::new(link.target.clone()).register(area, link_hits);
        }
    }
    draw_scrollbar(frame, inner, app.content_scroll, rows.len(), theme);
    hits
}

pub(super) fn pull_request_content_links(
    app: &App,
    showing_check: bool,
    rows: &[ContentRow],
) -> Vec<ContentLink> {
    if showing_check {
        let Some(check) = app
            .selected_pull_request_check()
            .filter(|check| !check.link.is_empty())
        else {
            return Vec::new();
        };
        let target = OpenTarget::Browser(check.link.clone());
        let url_row = 2
            + usize::from(!check.started_at.is_empty())
            + usize::from(!check.description.is_empty());
        return vec![
            ContentLink {
                row: 0,
                start: 2,
                width: check.name.width(),
                target: target.clone(),
            },
            ContentLink {
                row: url_row,
                start: DETAIL_LABEL_WIDTH,
                width: check.link.width(),
                target,
            },
        ];
    }
    let Some(pull_request) = app.selected_pull_request() else {
        return Vec::new();
    };
    let mut links = Vec::new();
    let number = format!("#{}", pull_request.number);
    if !pull_request.url.is_empty() {
        let target = OpenTarget::Browser(pull_request.url.clone());
        push_content_link(rows, &mut links, 0, &number, target.clone());
        push_content_link(rows, &mut links, 0, &pull_request.title, target.clone());
        push_content_link(rows, &mut links, 6, &pull_request.url, target);
    }
    let author = format!("@{}", pull_request.author);
    if let Some(target) = app.account_open_target(&pull_request.author) {
        push_content_link(rows, &mut links, 1, &author, target);
    }
    if let Some(target) = app.pull_request_head_branch_open_target() {
        push_content_link(rows, &mut links, 2, &pull_request.head_label(), target);
    }
    if let Some(target) = app.pull_request_base_branch_open_target() {
        push_content_link(rows, &mut links, 3, &pull_request.base_label(), target);
    }

    let mut conversation_row = rows
        .iter()
        .position(|row| content_row_text(row).contains("Conversation"))
        .unwrap_or(7);
    for entry in &app.pull_request_conversation.entries {
        let actor = format!("@{}", entry.actor);
        let actor_location = find_conversation_actor_link(rows, conversation_row, &actor);
        if let Some((row, start, width)) = actor_location {
            if let Some(target) = app.account_open_target(&entry.actor) {
                links.push(ContentLink {
                    row,
                    start,
                    width,
                    target,
                });
            }
            conversation_row = row.saturating_add(1);
            if !entry.url.is_empty() {
                let action = rows.get(row).and_then(|content| {
                    let action = content.line.spans.get(2)?;
                    let action_start = content.line.spans.iter().take(2).map(Span::width).sum();
                    Some((action_start, action.width()))
                });
                if let Some((action_start, action_width)) = action {
                    links.push(ContentLink {
                        row,
                        start: action_start,
                        width: action_width,
                        target: OpenTarget::Browser(entry.url.clone()),
                    });
                }
            }
        }
    }
    links
}

pub(super) fn find_conversation_actor_link(
    rows: &[ContentRow],
    start_row: usize,
    actor: &str,
) -> Option<(usize, usize, usize)> {
    rows.iter()
        .enumerate()
        .skip(start_row)
        .find_map(|(row, content)| {
            let actor_span = content.line.spans.get(1)?;
            (actor_span.content == actor).then(|| {
                let start = content.line.spans.first().map_or(0, Span::width);
                (row, start, actor_span.width())
            })
        })
}

pub(super) fn push_content_link(
    rows: &[ContentRow],
    links: &mut Vec<ContentLink>,
    start_row: usize,
    text: &str,
    target: OpenTarget,
) {
    if let Some((row, start, width)) = find_content_link(rows, start_row, text) {
        links.push(ContentLink {
            row,
            start,
            width,
            target,
        });
    }
}

pub(super) fn find_content_link(
    rows: &[ContentRow],
    start_row: usize,
    needle: &str,
) -> Option<(usize, usize, usize)> {
    if needle.is_empty() {
        return None;
    }
    rows.iter()
        .enumerate()
        .skip(start_row)
        .find_map(|(row, content)| {
            let text = content_row_text(content);
            let byte_start = text.find(needle)?;
            Some((row, text.get(..byte_start)?.width(), needle.width()))
        })
}

pub(super) fn content_row_text(row: &ContentRow) -> String {
    row.line.spans.iter().fold(String::new(), |mut text, span| {
        text.push_str(&span.content);
        text
    })
}

pub(super) fn overview_title(app: &App, showing_check: bool) -> String {
    let Some(pull_request) = app.selected_pull_request() else {
        return " Pull Request ".to_owned();
    };
    if showing_check {
        let name = app
            .selected_pull_request_check()
            .map_or("Check", |check| check.name.as_str());
        let loading = if app.pull_request_check_log_loading {
            "  · loading"
        } else {
            ""
        };
        return format!(" PR #{} · {name}{loading} ", pull_request.number);
    }
    let state = if pull_request.is_draft {
        "DRAFT"
    } else {
        pull_request.state.as_str()
    };
    let loading = if app.pull_request_conversation_loading {
        "  · loading"
    } else if app.pull_request_served_from_cache() {
        "  · cached"
    } else {
        ""
    };
    format!(" PR #{} · {state}{loading} ", pull_request.number)
}
