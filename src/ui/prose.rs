#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProseStyle {
    Text,
    Heading,
    Bullet,
    Code,
    Quote,
}

pub(super) fn prose_style(style: ProseStyle, theme: &Theme) -> Style {
    match style {
        ProseStyle::Text | ProseStyle::Bullet => Style::default().fg(theme.text),
        ProseStyle::Heading => Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ProseStyle::Code => Style::default().fg(theme.modified),
        ProseStyle::Quote => Style::default().fg(theme.muted),
    }
}

#[doc = " Render a body under its header behind a continuous gutter, so a long comment"]
#[doc = " stays visibly attached to the person who wrote it. Code carries a second"]
#[doc = " marker because it is the one kind of line that is not wrapped to the pane."]
pub(super) fn push_prose(
    rows: &mut Vec<ContentRow>,
    body: &str,
    gutter: &str,
    width: usize,
    theme: &Theme,
) {
    let available = width.saturating_sub(gutter.width() + 2);
    for (style, text) in wrap_prose(body, available) {
        let code = style == ProseStyle::Code;
        let line = Line::from(vec![
            Span::styled(
                format!("{gutter}{}", if code { " ▏ " } else { "  " }),
                Style::default().fg(theme.border),
            ),
            Span::styled(text, prose_style(style, theme)),
        ]);
        rows.push(if code {
            ContentRow::wide(line)
        } else {
            ContentRow::plain(line)
        });
    }
}

#[doc = " Window a composed row horizontally, in display columns rather than bytes, so"]
#[doc = " wide code and log lines can be read past the edge of the pane."]
pub(super) fn shift_line(line: &Line<'static>, skip: usize, width: usize) -> Line<'static> {
    let mut spans = Vec::with_capacity(line.spans.len());
    let mut scanned = 0;
    let mut used = 0;
    for span in &line.spans {
        if used >= width {
            break;
        }
        let span_width = span.content.width();
        if scanned + span_width <= skip {
            scanned += span_width;
            continue;
        }
        let text = slice_width(&span.content, skip.saturating_sub(scanned), width - used);
        scanned += span_width;
        if text.is_empty() {
            continue;
        }
        used += text.width();
        spans.push(Span::styled(text, span.style));
    }
    Line::from(spans)
}

#[expect(
    clippy::option_if_let_else,
    reason = "the branch is one arm of a longer chain that map_or_else cannot express"
)]
#[doc = " Wrap a Markdown body to a fixed width, keeping paragraph breaks, list"]
#[doc = " structure and fenced code intact. Code is truncated rather than wrapped so"]
#[doc = " its own indentation still reads correctly."]
pub(super) fn wrap_prose(value: &str, width: usize) -> Vec<(ProseStyle, String)> {
    let width = width.max(8);
    let mut output = Vec::new();
    let mut fenced = false;
    let mut previous_blank = true;
    for raw_line in value.lines() {
        let trimmed = raw_line.trim_end();
        if trimmed.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            previous_blank = false;
            output.push((ProseStyle::Code, trimmed.to_owned()));
            continue;
        }
        if trimmed.trim().is_empty() {
            if !previous_blank {
                output.push((ProseStyle::Text, String::new()));
                previous_blank = true;
            }
            continue;
        }
        previous_blank = false;
        let content = trimmed.trim_start();
        let (style, indent, body) = if let Some(rest) = content.strip_prefix("> ") {
            (ProseStyle::Quote, "  ", rest)
        } else if content.starts_with('#') {
            (
                ProseStyle::Heading,
                "",
                content.trim_start_matches('#').trim_start(),
            )
        } else if let Some(rest) = ["- ", "* ", "+ "]
            .into_iter()
            .find_map(|marker| content.strip_prefix(marker))
        {
            (ProseStyle::Bullet, "  ", rest)
        } else {
            (ProseStyle::Text, "", content)
        };
        let prefix = if style == ProseStyle::Bullet {
            "• "
        } else {
            ""
        };
        let available = width.saturating_sub(indent.width() + prefix.width());
        let body = body.replace('*', "");
        for (index, wrapped) in wrap_words(&body, available).into_iter().enumerate() {
            let lead = if index == 0 {
                format!("{indent}{prefix}")
            } else {
                " ".repeat(indent.width() + prefix.width())
            };
            output.push((style, format!("{lead}{wrapped}")));
        }
    }
    while output.last().is_some_and(|(_, text)| text.is_empty()) {
        drop(output.pop());
    }
    output
}

pub(super) fn wrap_words(value: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in value.split_whitespace() {
        let word_width = word.width();
        if current.is_empty() {
            current = if word_width > width {
                lines.push(truncate_end(word, width));
                continue;
            } else {
                word.to_owned()
            };
        } else if current.width() + 1 + word_width <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            if word_width > width {
                lines.push(truncate_end(word, width));
            } else {
                word.clone_into(&mut current);
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
